#!/usr/bin/env python3
"""Matched shared Dream/reopen operation regression alongside history profiles."""
import argparse,json,os,pathlib,statistics
import freshness_p2_acceptance as guard

def main():
 p=argparse.ArgumentParser(description=__doc__)
 p.add_argument('--baseline',type=pathlib.Path,required=True)
 p.add_argument('--candidate',type=pathlib.Path,required=True)
 p.add_argument('--out',type=pathlib.Path,required=True)
 p.add_argument('--rounds',type=int,default=3)
 args=p.parse_args();args.out.mkdir(parents=True,exist_ok=True);data=[]
 guard.QUIET_REQUIRED=True
 variants={'baseline':args.baseline,'candidate':args.candidate}
 for round in range(args.rounds):
  order=list(variants) if round%2==0 else list(reversed(variants));identities=[]
  for label in order:
   name=f'operation-{label}-round{round}';print(name,flush=True)
   out=args.out/(name+'.stdout.jsonl');err=args.out/(name+'.stderr.jsonl')
   env={**os.environ,'RELIQUARY_FRESHNESS_TRACE':'1'}
   env.pop('RELIQUARY_FRESHNESS_HISTORY_DEPTH',None);env.pop('RELIQUARY_FRESHNESS_HISTORY_BATCH',None)
   guard.run([str(variants[label])],out,err,env=env)
   rows=[json.loads(line) for line in out.read_text(encoding='utf-8').splitlines() if line.startswith('{')]
   identities.append({r['stage']:(r['event_count'],r['root_count']) for r in rows})
   data.append({'variant':label,'round':round,'samples':rows})
  assert identities[0]==identities[1], 'Operation event/root shapes changed'
 (args.out/'operation-raw.json').write_text(json.dumps(data,indent=2),encoding='utf-8')
 summary=[]
 for stage in [r['stage'] for r in data[0]['samples']]:
  values={label:[next(r['elapsed_ns'] for r in sample['samples'] if r['stage']==stage) for sample in data if sample['variant']==label] for label in variants}
  baseline=statistics.median(values['baseline']);candidate=statistics.median(values['candidate'])
  summary.append({'stage':stage,'baseline_p50_ns':baseline,'candidate_p50_ns':candidate,'candidate_over_baseline':candidate/baseline,'samples':values})
 (args.out/'operation-summary.json').write_text(json.dumps(summary,indent=2),encoding='utf-8')
 (args.out/'operation-load.json').write_text(json.dumps(guard.LOAD_OBSERVATIONS,indent=2),encoding='utf-8')
 print('Shared operation comparisons complete',flush=True)
if __name__=='__main__':main()
