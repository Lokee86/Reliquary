#!/usr/bin/env python3
"""Read-only size comparison over exact native scorer audit rows.

No runtime codec, corpus write, or format selection. Run after the explicit Rust
regeneration audit. Missing/error outcomes are rejected, not counted as parity.
"""
from __future__ import annotations
import argparse
from collections import defaultdict
import json
from pathlib import Path
from measure_phase3_settlement import uleb_size

ROOT=Path(__file__).resolve().parents[1]

def zigzag(n):return (n<<1)^(n>>31)
def sequence_bytes(rows):
    total=uleb_size(len(rows));previous=-1
    for index,amount in rows:
        assert index>previous and amount!=0
        total+=uleb_size(index-previous)+uleb_size(zigzag(amount));previous=index
    return total

def measure(fixture):
    if fixture.get("status"):raise ValueError(f"unavailable regeneration: {fixture['fixture']}: {fixture.get('error')}")
    basis=fixture['causal_basis'];initial=basis['initial_records'];rows=basis['ordered_scorer_rows']
    ids=sorted(r['memory_id'] for r in initial);ii={v:i for i,v in enumerate(ids)}
    events=sorted({(r['position'],r['event_id']) for r in rows});ei={v:i for i,v in enumerate(events)}
    recipient=defaultdict(list);event=defaultdict(list);seen=set()
    for r in rows:
        key=(r['recipient'],r['event_id'])
        if key in seen:raise ValueError('duplicate scorer event/recipient')
        seen.add(key);assert -100<=r['amount']<=100 and r['amount']!=0
        recipient[r['recipient']].append((ei[(r['position'],r['event_id'])],r['amount']))
        event[(r['position'],r['event_id'])].append((ii[r['recipient']],r['amount']))
    profiles=sorted({tuple(sorted(recipient.get(memory,[]))) for memory in ids});pi={v:i for i,v in enumerate(profiles)}
    profile_bytes=uleb_size(len(profiles))+sum(sequence_bytes(p) for p in profiles)
    refs=uleb_size(len(ids))+sum(uleb_size(pi[tuple(sorted(recipient.get(memory,[])))]) for memory in ids)
    event_bytes=uleb_size(len(events))+sum(sequence_bytes(sorted(event[e])) for e in events)
    recipient_bytes=uleb_size(len(ids))+sum(sequence_bytes(sorted(recipient.get(memory,[]))) for memory in ids)
    # Exact anchor values are independently dictionary encoded. The serialized
    # causal authority contains no output postimage map.
    anchors=[]
    for r in initial:
        v=r['value'];anchors.append((v['score'],v['admitted_at_turn'],v['accounted_turn']))
    unique=sorted(set(anchors),key=lambda v:(v[0],-1 if v[1] is None else v[1],-1 if v[2] is None else v[2]));ai={v:i for i,v in enumerate(unique)}
    anchor_table=uleb_size(len(unique))+sum(4+(8 if v[1] is not None else 0)+(8 if v[2] is not None else 0) for v in unique)
    anchor_refs=uleb_size(len(anchors))+sum(uleb_size(ai[v]) for v in anchors)
    common=uleb_size(len(ids))+32*len(ids)+anchor_table+anchor_refs+uleb_size(len(events))+sum(32+uleb_size(position) for position,_ in events)
    return dict(fixture=fixture['fixture'],residual_record_count=fixture['residual_record_count'],nonrecord_fields_equal=fixture['settlement_nonrecord_fields_equal'],record_keys=len(ids),scorer_rows=len(rows),unique_events=len(events),unique_recipient_profiles=len(profiles),unique_initial_records=len(unique),common_key_anchor_event_bytes=common,recipient_rows_bytes=common+recipient_bytes,event_recipient_rows_bytes=common+event_bytes,deduplicated_recipient_profiles_bytes=common+profile_bytes+refs,scope='causal basis only; excludes semantic scope and identity/digest metadata; no format selected')

def main():
    p=argparse.ArgumentParser();p.add_argument('--input',type=Path,default=ROOT/'.verification-tmp/phase3-s3-regeneration/phase3-s3-regeneration-audit.json');p.add_argument('--output',type=Path,default=ROOT/'.verification-tmp/phase3-s3-causal-size-comparison.json');a=p.parse_args()
    original=a.input.read_bytes();report=json.loads(original);rows=[measure(f) for f in report['fixtures']]
    result=dict(status='measurement_only_not_selected',files=rows);a.output.write_text(json.dumps(result,indent=2)+'\n');assert a.input.read_bytes()==original;print(json.dumps(result,indent=2))
if __name__=='__main__':main()
