use super::{ProductError, Value, invalid, json};
use std::io::{self, BufRead, Write};

const MAX_LINE: usize = 16 * 1024;
const MAX_ID: usize = 128;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Command {
    Query(String),
    Refine(String),
    Traverse(Vec<String>),
    Show(String),
    Source(String),
    Neighbors(String),
    Status,
    NewTurn,
    Quit,
}

fn command(name: &str, argument: Value) -> Result<Command, ProductError> {
    let text = || {
        argument
            .as_str()
            .filter(|text| !text.trim().is_empty())
            .map(str::to_owned)
            .ok_or_else(|| invalid("command requires a nonempty string"))
    };
    match name {
        "query" => Ok(Command::Query(text()?)),
        "refine" => Ok(Command::Refine(text()?)),
        "traverse" => {
            let ids = argument
                .as_array()
                .ok_or_else(|| invalid("traverse requires Memory IDs"))?;
            if ids.is_empty() || ids.len() > 10 {
                return Err(invalid("traverse requires between one and ten Memory IDs"));
            }
            Ok(Command::Traverse(
                ids.iter()
                    .map(|id| {
                        id.as_str()
                            .map(str::to_owned)
                            .ok_or_else(|| invalid("invalid Memory ID"))
                    })
                    .collect::<Result<_, _>>()?,
            ))
        }
        "show" => Ok(Command::Show(text()?)),
        "source" => Ok(Command::Source(text()?)),
        "neighbors" => Ok(Command::Neighbors(text()?)),
        "status" if argument.is_null() => Ok(Command::Status),
        "new-turn" if argument.is_null() => Ok(Command::NewTurn),
        "quit" if argument.is_null() => Ok(Command::Quit),
        _ => Err(invalid(
            "unknown interaction command or unexpected argument",
        )),
    }
}

fn json_command(line: &str) -> (Value, Result<Command, ProductError>) {
    let Ok(Value::Object(mut record)) = serde_json::from_str::<Value>(line) else {
        return (Value::Null, Err(invalid("invalid JSONL command record")));
    };
    let id = match record.remove("request_id") {
        Some(id @ Value::Number(_)) => id,
        Some(Value::String(id)) if !id.is_empty() && id.len() <= MAX_ID => Value::String(id),
        _ => return (Value::Null, Err(invalid("invalid request_id"))),
    };
    let result = (|| {
        let name = record
            .remove("command")
            .ok_or_else(|| invalid("command is required"))?;
        let name = name
            .as_str()
            .ok_or_else(|| invalid("command must be a string"))?;
        let field = match name {
            "query" | "refine" => "text",
            "show" | "source" | "neighbors" => "memory_id",
            "traverse" => "memory_ids",
            _ => "",
        };
        let argument = record.remove(field).unwrap_or(Value::Null);
        if !record.is_empty() {
            return Err(invalid("unexpected command field"));
        }
        command(name, argument)
    })();
    (id, result)
}

fn text_command(line: &str) -> Result<Command, ProductError> {
    let (name, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
    let rest = rest.trim();
    let argument = if rest.is_empty() {
        Value::Null
    } else if name == "traverse" {
        json!(rest.split_whitespace().collect::<Vec<_>>())
    } else if rest.starts_with('"') {
        Value::String(serde_json::from_str(rest).map_err(|_| invalid("invalid quoted argument"))?)
    } else {
        json!(rest)
    };
    command(name, argument)
}

// Read one record while capping allocation even when an input has no newline.
fn read_line(input: &mut dyn BufRead, bytes: &mut Vec<u8>) -> io::Result<Option<bool>> {
    bytes.clear();
    let mut overflow = false;
    loop {
        let chunk = input.fill_buf()?;
        if chunk.is_empty() {
            return Ok((overflow || !bytes.is_empty()).then_some(overflow));
        }
        let newline = chunk.iter().position(|byte| *byte == b'\n');
        let len = newline.map_or(chunk.len(), |index| index + 1);
        if !overflow && bytes.len() + len <= MAX_LINE {
            bytes.extend_from_slice(&chunk[..len]);
        } else {
            overflow = true;
        }
        input.consume(len);
        if newline.is_some() {
            return Ok(Some(overflow));
        }
    }
}

pub(crate) fn run(
    mut input: impl BufRead,
    mut output: impl Write,
    jsonl: bool,
    mut execute: impl FnMut(Command) -> Result<Value, ProductError>,
) -> Result<bool, ProductError> {
    let mut bytes = Vec::new();
    let mut success = true;
    while let Some(overflow) = read_line(&mut input, &mut bytes)? {
        let (id, request) = match std::str::from_utf8(&bytes) {
            _ if overflow => (Value::Null, Err(invalid("interaction line exceeds 16 KiB"))),
            Err(_) => (Value::Null, Err(invalid("interaction line is not UTF-8"))),
            Ok(line) if line.trim().is_empty() && !jsonl => continue,
            Ok(line) if jsonl => json_command(line),
            Ok(line) => (Value::Null, text_command(line.trim())),
        };
        let quit = matches!(request, Ok(Command::Quit));
        let result = request.and_then(|command| {
            if quit {
                Ok(json!({"operation":"quit"}))
            } else {
                execute(command)
            }
        });
        let mut reply = match result {
            Ok(value) => json!({"result":value}),
            Err(error) => {
                success = false;
                json!({"error":super::error_json(&error)})
            }
        };
        if jsonl {
            reply["request_id"] = id;
        }
        writeln!(output, "{reply}")?;
        output.flush()?;
        if quit {
            break;
        }
    }
    Ok(success)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_recovers_after_bad_record_and_echoes_request_ids_in_order() {
        let mut input = format!("{}\n", "x".repeat(MAX_LINE));
        input.push_str(
            "{\"request_id\":1,\"command\":\"refine\"}\n\
             {\"request_id\":2,\"command\":\"query\",\"text\":\"a b\"}\n\
             {\"request_id\":3,\"command\":\"quit\"}\n\
             {\"request_id\":4,\"command\":\"status\"}\n",
        );
        let mut output = Vec::new();
        assert!(
            !run(input.as_bytes(), &mut output, true, |command| {
                assert_eq!(command, Command::Query("a b".into()));
                Ok(json!({}))
            })
            .unwrap()
        );
        let replies = String::from_utf8(output)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(replies.len(), 4);
        assert!(replies[0]["request_id"].is_null());
        assert_eq!(replies[1]["error"]["kind"], "invalid_input");
        assert_eq!(replies[2]["request_id"], 2);
        assert_eq!(replies[3]["result"]["operation"], "quit");
    }
}
