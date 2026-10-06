import json,pathlib,hashlib
d=pathlib.Path(r'C:\Users\archa\AppData\Local\Temp\reliquary-freshness-perf-baseline\freshness-evidence')
for n in ['synthetic','14d','28d','ellis']:
 p=d/f'p0-propagation-{n}.json'
 x=json.loads(p.read_text(encoding='utf-8-sig'))
 print('fixture',n,'sha256',hashlib.sha256(p.read_bytes()).hexdigest(),'graph_nodes',x['graph']['node_count'],'edges',x['graph']['undirected_active_edges'],'community',x['graph']['snapshot_validity'],'source_verified',x.get('source_hash_unchanged'))
 for ident in ['accepted_batch_1','accepted_batch_16','dream_roots_8_same','dream_roots_8_across','dream_roots_8_unknown']:
  rows=[r for r in x['results'] if r['scenario_id']==ident and r['requested_workers'] in (1,4,8)]
  if rows:
   print(' ',ident,' '.join(f"w{r['requested_workers']}={r['operation_latency_ns']['p50']/1e6:.3f}ms/p95={r['operation_latency_ns']['p95']/1e6:.3f}ms recipients={sum(z['candidate_count'] for z in r['correctness_reference_events'])}" for r in rows))
print('OP HASH',hashlib.sha256((d/'p0-operation.stdout.jsonl').read_bytes()).hexdigest(),'TRACE HASH',hashlib.sha256((d/'p0-operation.stderr.jsonl').read_bytes()).hexdigest())