"""Measurement-only causal + irreducible score correction wire prototype."""
import sys,struct,json,hashlib,time,tracemalloc
from pathlib import Path
sys.path.insert(0,str(Path.cwd()/"scripts"))
import measure_phase3_settlement as audit
from prototype_phase3_settlement_dictionary import var,R
DOMAIN=b"reliquary-phase3-causal-prototype-v1"
def zig(n):return (n<<1)^(n>>31)
def unzig(n):return (n>>1)^-(n&1)
def record(v):
    out=struct.pack('<h',v['score'])
    for k in ['admitted_at_turn','accounted_turn']:
        out+=b'\0' if v[k] is None else b'\1'+struct.pack('<Q',v[k])
    return out

def encode(p,raw,f):
    d=p['data'];ids=d['memory_ids'];ii={v:i for i,v in enumerate(ids)}
    texts=sorted(v.encode() for v in d['strings']);ti={v.decode():i for i,v in enumerate(texts)}
    sets=sorted({tuple(d[k]) for k in ['born','affected','cohort']}|{tuple(r['id'] for r in d['records'])});si={v:i for i,v in enumerate(sets)}
    out=bytearray(b'CVAS3P07'+raw[8:297]);out+=var(len(ids))+b''.join(ids)+var(len(texts));prior=b''
    for value in texts:
        n=0
        while n<min(len(prior),len(value)) and prior[n]==value[n]:n+=1
        out+=var(n)+var(len(value)-n)+value[n:];prior=value
    out+=var(len(sets))
    for values in sets:
        out+=var(len(values));previous=-1
        for v in values:idx=ii[v];out+=var(idx-previous);previous=idx
    out+=var(si[tuple(d['born'])])+var(si[tuple(d['affected'])])+var(len(d['sources']))
    for c,n in d['sources']:out+=var(ti[c])+var(ti[n])
    out+=var(len(d['events']))
    for e in d['events']:out+=var(ti[e['id']])+e['digest']+struct.pack('<Q',e['turn'])
    bits=bytearray((len(d['events'])+7)//8);sup=set(d['superseded'])
    for i,e in enumerate(d['events']):
        if e['id'] in sup:bits[i//8]|=1<<(i%8)
    out+=bits+var(si[tuple(d['cohort'])])+var(si[tuple(r['id'] for r in d['records'])])
    basis=f['causal_basis'];initial={bytes.fromhex(r['memory_id']):record(r['value']) for r in basis['initial_records']};keys=tuple(r['id'] for r in d['records']);assert set(initial)==set(keys)
    values=sorted(set(initial.values()));vi={v:i for i,v in enumerate(values)};out+=var(len(values))
    for v in values:out+=var(len(v))+v
    out+=var(len(keys))
    for key in keys:out+=var(vi[initial[key]])
    rows=basis['ordered_scorer_rows'];events=sorted({(r['position'],r['event_id']) for r in rows});ei={v:i for i,v in enumerate(events)};byevent={i:[] for i in range(len(events))}
    for r in rows:byevent[ei[(r['position'],r['event_id'])]].append((ii[bytes.fromhex(r['recipient'])],r['amount']))
    out+=var(len(events));previous_position=0
    for i,(position,event_id) in enumerate(events):
        out+=var(position-previous_position);previous_position=position;v=sorted(byevent[i]);out+=var(len(v));previous=-1
        for idx,amount in v:assert idx>previous;out+=var(idx-previous)+var(zig(amount));previous=idx
    out+=var(len(f['residual_records']));previous=-1
    for delta in f['residual_records']:
        old,new=delta['original'],delta['regenerated'];assert old and new
        assert old['admitted_at_turn']==new['admitted_at_turn'] and old['accounted_turn']==new['accounted_turn']
        idx=ii[bytes.fromhex(delta['memory_id'])];out+=var(idx-previous)+var(zig(old['score']-new['score']));previous=idx
    out+=hashlib.sha256(DOMAIN+out).digest();return bytes(out)

def decode(data,leaves):
    if hashlib.sha256(DOMAIN+data[:-32]).digest()!=data[-32:]:raise ValueError('checksum')
    r=R(data[:-32]);fixed=r.take(297);assert fixed[:8]==b'CVAS3P07';out=bytearray(b'CVAFRS06'+fixed[8:]);cut=struct.unpack_from('<Q',fixed,256)[0]
    ids=[r.take(32) for _ in range(r.var())];assert ids==sorted(set(ids));texts=[];prior=b''
    for _ in range(r.var()):
        n=r.var();assert n<=len(prior);v=prior[:n]+r.take(r.var());v.decode();texts.append(v);prior=v
    assert texts==sorted(set(texts));sets=[]
    for _ in range(r.var()):
        values=[];previous=-1
        for _ in range(r.var()):
            delta=r.var();assert delta>0;previous+=delta;values.append(ids[previous])
        sets.append(values)
    def idset(v):return struct.pack('<I',len(v))+b''.join(v)
    def text(v):return struct.pack('<I',len(v))+v
    out+=idset(sets[r.var()])+idset(sets[r.var()]);n=r.var();out+=struct.pack('<I',n)
    for _ in range(n):out+=text(texts[r.var()])+text(texts[r.var()])
    n=r.var();out+=struct.pack('<I',n);event_ids=[]
    for _ in range(n):
        key=texts[r.var()].decode();digest=r.take(32);turn=r.take(8);leaf=leaves[key]
        assert leaf['digest']==digest and struct.pack('<Q',leaf['turn'])==turn
        out+=text(key.encode())+digest+turn+idset(leaf['recipients']);event_ids.append(key)
    bits=r.take((n+7)//8)
    if n%8:assert bits[-1]>>(n%8)==0
    sup=[key for i,key in enumerate(event_ids) if bits[i//8]&(1<<(i%8))];out+=struct.pack('<I',len(sup))+b''.join(text(v.encode()) for v in sup)
    out+=idset(sets[r.var()]);keys=sets[r.var()];values=[r.take(r.var()) for _ in range(r.var())];assert values==sorted(set(values));assert r.var()==len(keys)
    records={}
    for key in keys:
        value=values[r.var()];z=R(value);score=struct.unpack('<h',z.take(2))[0];anchors=[]
        for _ in range(2):
            tag=z.take(1)[0];assert tag in (0,1);anchors.append(struct.unpack('<Q',z.take(8))[0] if tag else None)
        assert z.at==len(value);records[key]=[score,*anchors]
    def settle(key,position):
        v=records[key];accounted=v[2]
        if accounted is not None:
            assert position>=accounted;steps=(position-accounted)//10;v[0]=max(-100,v[0]-steps);v[2]+=steps*10
    position=0
    for event_index in range(r.var()):
        position+=r.var();assert 0<position<=cut;previous=-1
        for _ in range(r.var()):
            delta=r.var();assert delta>0;previous+=delta;key=ids[previous];amount=unzig(r.var());assert -100<=amount<=100 and amount!=0
            settle(key,position);records[key][0]=min(100,records[key][0]+amount) if amount>0 else max(-100,records[key][0]+amount)
    for key in keys:settle(key,cut)
    previous=-1
    for _ in range(r.var()):
        delta=r.var();assert delta>0;previous+=delta;key=ids[previous];correction=unzig(r.var());assert correction!=0;records[key][0]+=correction;assert -100<=records[key][0]<=100
    assert r.at==len(r.data);out+=struct.pack('<I',len(keys))
    for key in keys:
        score,admitted,accounted=records[key];out+=key+record(dict(score=score,admitted_at_turn=admitted,accounted_turn=accounted))
    out+=hashlib.sha256(out).digest();return bytes(out)

def main():
    source=Path('.verification-tmp/phase3-s3-regeneration/phase3-s3-regeneration-audit.json');report=json.loads(source.read_text());baseline=json.loads(audit.BASELINE.read_text());result=[]
    for f in report['fixtures']:
        assert not f.get('status')
        entry=next(v for v in baseline['canonical_sources'] if v['fixture']==f['fixture']);path=Path('.verification-tmp/phase3-before')/f['fixture'];allbytes=path.read_bytes();assert hashlib.sha256(allbytes).hexdigest()==entry['sha256'];at=f['settlement_frame_offset'];n=f['original_settlement_payload_bytes'];raw=allbytes[at+8:at+8+n];parsed=audit.parse_settlement(raw,f['fixture'],at,n+8,0)
        leaves={v['id']:v for v in parsed['data']['events']}
        tracemalloc.start();start=time.perf_counter();encoded=encode(parsed,raw,f);enc_time=time.perf_counter()-start;_,enc_peak=tracemalloc.get_traced_memory();tracemalloc.stop()
        tracemalloc.start();start=time.perf_counter();decoded=decode(encoded,leaves);dec_time=time.perf_counter()-start;_,dec_peak=tracemalloc.get_traced_memory();tracemalloc.stop();assert decoded==raw
        assert hashlib.sha256(path.read_bytes()).hexdigest()==entry['sha256']
        result.append(dict(fixture=f['fixture'],candidate_bytes=len(encoded),original_bytes=n,exact_original_payload_roundtrip=True,score_corrections=len(f['residual_records']),encode_seconds=enc_time,decode_seconds=dec_time,python_encode_peak_bytes=enc_peak,python_decode_peak_bytes=dec_peak,independent_journal_leaf_proof=f.get('original_event_leaves_validated_against_prefix_journal',False)))
    out=dict(status='measurement_prototype_not_selected',files=result,total_candidate_bytes=sum(r['candidate_bytes'] for r in result));Path('.verification-tmp/phase3-s3-causal-roundtrip.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out,indent=2))
if __name__=='__main__':main()
