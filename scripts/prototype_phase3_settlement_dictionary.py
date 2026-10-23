"""Read-only full-authority measurement control; not a runtime codec or selected format.

Writes only .verification-tmp and verifies all frozen source hashes. Retains postimages,
so this alternative alone does not satisfy P3.9.02. Retire after C3 measurement."""
import sys, json, hashlib, struct, time, tracemalloc
from pathlib import Path
sys.path.insert(0, str(Path.cwd()/"scripts"))
import measure_phase3_settlement as audit

def var(n):
    out=bytearray()
    while n>=128: out.append((n&127)|128); n>>=7
    out.append(n); return bytes(out)

class R:
    def __init__(self,data):self.data=data;self.at=0
    def take(self,n):
        if n<0 or self.at+n>len(self.data):raise ValueError("truncated")
        b=self.data[self.at:self.at+n];self.at+=n;return b
    def var(self):
        n=0
        for shift in range(0,70,7):
            b=self.take(1)[0];n|=(b&127)<<shift
            if b<128:
                if shift and b==0:raise ValueError("noncanonical varint")
                return n
        raise ValueError("oversize varint")

def encode(parsed,original):
    d=parsed["data"];ids=d["memory_ids"];ii={v:i for i,v in enumerate(ids)}
    strings=sorted(v.encode() for v in d["strings"]);ti={v.decode():i for i,v in enumerate(strings)}
    sets=sorted({tuple(d[k]) for k in ["born","affected","cohort"]}|{tuple(v["id"] for v in d["records"])}|{tuple(e["recipients"]) for e in d["events"]})
    si={v:i for i,v in enumerate(sets)};vals=sorted(set(d["record_values"]));vi={v:i for i,v in enumerate(vals)}
    out=bytearray(original[:297]);out+=var(len(ids))+b"".join(ids)
    out+=var(len(strings));prior=b""
    for text in strings:
        prefix=0
        while prefix<min(len(prior),len(text)) and prior[prefix]==text[prefix]:prefix+=1
        suffix=text[prefix:];out+=var(prefix)+var(len(suffix))+suffix;prior=text
    out+=var(len(sets))
    for values in sets:
        out+=var(len(values));previous=0
        for v in values:idx=ii[v];out+=var(idx-previous);previous=idx
    out+=var(si[tuple(d["born"])])+var(si[tuple(d["affected"])])
    out+=var(len(d["sources"]))
    for c,n in d["sources"]:out+=var(ti[c])+var(ti[n])
    out+=var(len(d["events"]))
    for e in d["events"]:out+=var(ti[e["id"]])+e["digest"]+struct.pack("<Q",e["turn"])+var(si[tuple(e["recipients"])])
    out+=var(len(d["superseded"]))
    for e in d["superseded"]:out+=var(ti[e])
    out+=var(si[tuple(d["cohort"])])+var(si[tuple(v["id"] for v in d["records"])])+var(len(vals))
    for v in vals:out+=var(len(v))+v
    out+=var(len(d["records"]))
    for rec in d["records"]:out+=var(vi[rec["value_wire"]])
    out+=hashlib.sha256(out).digest();return bytes(out)

def decode(encoded):
    if hashlib.sha256(encoded[:-32]).digest()!=encoded[-32:]:raise ValueError("checksum")
    r=R(encoded[:-32]);out=bytearray(r.take(297));ids=[r.take(32) for _ in range(r.var())]
    texts=[];prior=b""
    for _ in range(r.var()):
        prefix=r.var()
        if prefix>len(prior):raise ValueError("prefix")
        prior=prior[:prefix]+r.take(r.var());prior.decode();texts.append(prior)
    sets=[]
    for _ in range(r.var()):
        values=[];previous=0
        for j in range(r.var()):
            previous+=r.var()
            if j and previous<=values[-1][0]:raise ValueError("ID order")
            values.append((previous,ids[previous]))
        sets.append([v for _,v in values])
    def ids_field(values):return struct.pack("<I",len(values))+b"".join(values)
    def text(value):return struct.pack("<I",len(value))+value
    out+=ids_field(sets[r.var()])+ids_field(sets[r.var()])
    count=r.var();out+=struct.pack("<I",count)
    for _ in range(count):out+=text(texts[r.var()])+text(texts[r.var()])
    count=r.var();out+=struct.pack("<I",count)
    for _ in range(count):out+=text(texts[r.var()])+r.take(32)+r.take(8)+ids_field(sets[r.var()])
    count=r.var();out+=struct.pack("<I",count)
    for _ in range(count):out+=text(texts[r.var()])
    out+=ids_field(sets[r.var()]);keys=sets[r.var()];vals=[r.take(r.var()) for _ in range(r.var())]
    count=r.var()
    if count!=len(keys):raise ValueError("record key count")
    out+=struct.pack("<I",count)
    for key in keys:out+=key+vals[r.var()]
    if r.at!=len(r.data):raise ValueError("trailing")
    out+=hashlib.sha256(out).digest();return bytes(out)

def main():
    baseline=json.loads(audit.BASELINE.read_text());rows=[]
    for source in baseline["canonical_sources"]:
        path=Path(".verification-tmp/phase3-before")/source["fixture"];data=path.read_bytes()
        assert hashlib.sha256(data).hexdigest()==source["sha256"]
        if not str(path).endswith(".rel"):continue
        field=json.loads(Path("benchmarks/dependency-convergence-phase3-settlement-field-audit-2026-10-22.json").read_text())
        f=next(v for v in field["files"] if v["fixture"]==source["fixture"])
        for entry in f["settlements"]:
            offset=entry["frame_offset"];length=entry["payload_bytes"];payload=data[offset+8:offset+8+length]
            parsed=audit.parse_settlement(payload,source["fixture"],offset,length+8,0)
            tracemalloc.start();start=time.perf_counter();encoded=encode(parsed,payload);encode_seconds=time.perf_counter()-start
            _,encode_peak=tracemalloc.get_traced_memory();tracemalloc.stop()
            tracemalloc.start();start=time.perf_counter();decoded=decode(encoded);decode_seconds=time.perf_counter()-start
            _,decode_peak=tracemalloc.get_traced_memory();tracemalloc.stop()
            assert decoded==payload
            expected=audit.lossless_prefix_delta_size(parsed)["total_bytes"];assert len(encoded)==expected,(len(encoded),expected)
            # Mutated bytes must not survive authentication, independent of location.
            for pos in [0,len(encoded)//2,len(encoded)-1]:
                bad=bytearray(encoded);bad[pos]^=1
                try:decode(bytes(bad))
                except ValueError:pass
                else:raise AssertionError("corruption accepted")
            rows.append(dict(fixture=source["fixture"],current_bytes=length,candidate_bytes=len(encoded),exact_original_roundtrip=True,encode_seconds=encode_seconds,decode_seconds=decode_seconds,python_encode_peak_bytes=encode_peak,python_decode_peak_bytes=decode_peak,source_sha256=source["sha256"]))
        assert hashlib.sha256(path.read_bytes()).hexdigest()==source["sha256"]
    report=dict(status="control_prototype_only_not_selected",scope="full authority dictionary, retains all postimages and therefore does not alone satisfy P3.9.02",runtime_codec_changes=False,corpus_changes=False,files=rows)
    Path(".verification-tmp/phase3-s3-dictionary-roundtrip.json").write_text(json.dumps(report,indent=2)+"\n");print(json.dumps(report,indent=2))
if __name__=="__main__":main()
