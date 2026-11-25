#!/usr/bin/env python3
"""Linux replay baseline using synthetic checksummed frames; no provider calls."""
import json, hashlib, tempfile, pathlib, subprocess, time, copy, os
import argparse, shutil, signal, sys
parser=argparse.ArgumentParser(description="Synthetic replay-only diagnostic; not a capacity qualification")
parser.add_argument('--repo',type=pathlib.Path,default=pathlib.Path(__file__).resolve().parents[4])
parser.add_argument('--binary',type=pathlib.Path,required=True)
parser.add_argument('--counts',type=int,nargs='+',default=[1000])
parser.add_argument('--timeout-seconds',type=float,default=45)
args=parser.parse_args()
if sys.platform!='linux':
 parser.error('this diagnostic records Linux RSS units and requires Linux process groups')
if not 0<args.timeout_seconds<=180:
 parser.error('timeout must be positive and at most 180 seconds')
if any(count<1 or count>30000 for count in args.counts):
 parser.error('counts must be between 1 and 30000; the v1 journal is bounded')
repo=args.repo.resolve()
binary=args.binary.resolve()
sha=lambda b:'sha256:'+hashlib.sha256(b).hexdigest()
canonical=lambda v:json.dumps(v,sort_keys=True,separators=(',',':'),ensure_ascii=False).encode()
binary_hash=sha(binary.read_bytes())
def stop_probe(process):
 try: os.killpg(process.pid,signal.SIGKILL)
 except ProcessLookupError: pass
 process.communicate()
with tempfile.TemporaryDirectory(prefix='usage-scale-probe-') as synthetic_root:
 root=pathlib.Path(synthetic_root)
 source_binary=binary
 binary=root/'probe-ai-gent'
 shutil.copy2(source_binary,binary)
 assert sha(binary.read_bytes())==binary_hash, 'binary changed while pinning the probe'
 (root/'specs/046-task-usage-ledger').mkdir(parents=True)
 (root/'specs/046-task-usage-ledger/tasks.md').write_text('- [ ] T001 Build\n')
 reg=json.loads((repo/'rust/crates/ai-gent/tests/fixtures/agent_046/register.json').read_text())
 p=subprocess.run([str(binary),'--json','usage','--root',str(root),'register','--input','-'],input=json.dumps(reg),text=True,capture_output=True,check=True)
 registered=json.loads(p.stdout)['result']
 base=json.loads((repo/'specs/046-task-usage-ledger/contracts/examples/usage-valid.json').read_text())
 base.update(task=registered['task'],attempt_id=registered['attempt_id'],run_id=registered['run_id'],parent_run_id=registered['parent_run_id'],segment_id=registered['segments'][0]['segment_id'],phase=registered['phase'],host_session_id=None)
 base['identity']=dict(registered['segments'][0]['configuration'],identity_quality='reported',effective_provider='openai',effective_model='fixture-model')
 base['execution_scope']['beneficiaries']=[registered['task']]
 log=root/'.ai-gent/usage/journal/observations.jsonl'
 print(json.dumps({'binary_sha256':binary_hash,'cpu_count':os.cpu_count(),'profile':'existing debug binary, one run, distinct terminal requests, no receipts; not reference hardware qualification'}),flush=True)
 for count in args.counts:
  with log.open('wb') as output:
   for n in range(1,count+1):
    obs=copy.deepcopy(base);obs['event_id']=f'30000000-0000-4000-8000-{n:012}';obs['sequence']=n
    obs['source'].update(event_id=f'source-{n}',request_id=f'request-{n}',scope='turn_total')
    request={'schema_version':1,'request_id':f'40000000-0000-4000-8000-{n:012}','repository_id':registered['task']['repository_id'],'operation':'ingest','payload':obs}
    payload=canonical(request)
    output.write(canonical({'schema_version':1,'sequence':n,'payload_bytes':len(payload),'checksum':sha(payload),'payload':request})+b'\n')
  start=time.perf_counter()
  command=['python3','-c','import subprocess,resource,sys; p=subprocess.run(sys.argv[1:]); print("PROBE rss_kib="+str(resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss),file=sys.stderr); sys.exit(p.returncode)',str(binary),'--json','usage','--root',str(root),'replay']
  interrupted=[]
  previous_handler=signal.signal(signal.SIGINT,lambda signum,frame:interrupted.append(True))
  try:
   # Defer SIGINT during Popen construction until the cleanup guard owns the
   # detached child. Otherwise interruption between fork and __enter__ leaks it.
   with subprocess.Popen(command,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,start_new_session=True) as process:
    try:
     signal.signal(signal.SIGINT,previous_handler)
     if interrupted: raise KeyboardInterrupt
     stdout,stderr=process.communicate(timeout=args.timeout_seconds)
    except subprocess.TimeoutExpired:
     stop_probe(process)
     print(json.dumps({'events':count,'status':'timeout','timeout_seconds':args.timeout_seconds}),flush=True)
     raise SystemExit(124)
    except BaseException:
     stop_probe(process)
     raise
    result=subprocess.CompletedProcess(command,process.returncode,stdout,stderr)
  finally:
   signal.signal(signal.SIGINT,previous_handler)
  elapsed=time.perf_counter()-start
  value=json.loads(result.stdout)
  assert result.returncode==0,(result.returncode,result.stderr,value)
  assert value['result']['usage_sequence']==count
  assert value['result']['usage']['total_tokens']==count*1200
  print(json.dumps({'events':count,'log_bytes':log.stat().st_size,'bytes_per_frame':log.stat().st_size/count,'wall_seconds':round(elapsed,3),'time':result.stderr.strip()}),flush=True)
 print(json.dumps({'binary_unchanged':sha(binary.read_bytes())==binary_hash,'synthetic_data_removed_on_exit':True}),flush=True)
