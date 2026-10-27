#!/usr/bin/env python3
"""Real OS pipes, actual production Adapter+SDK, clone-private cached synthetic data.
No HTTP endpoint overrides, clipboard, browser or settings producers are invoked.
"""
import subprocess, pathlib, tempfile, json, hashlib, time, uuid, selectors, sys, os
BASE=pathlib.Path(__file__).resolve().parents[1]
BIN=pathlib.Path(sys.argv[1]).resolve() if len(sys.argv)>1 else BASE/'target/debug/country-lookup-native'
KEY='PUBLIC_SYNTHETIC_COUNTRY_TEST'
ID='com.tezbar.native.country-lookup'
SCRATCH=BASE/'qa/scratch';SCRATCH.mkdir(exist_ok=True)
class Pipe:
 def __init__(self,root):
  self.root=root;self.frames=[];self.pending={};self.buf=b''
  self.p=subprocess.Popen([str(BIN)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
  self.sel=selectors.DefaultSelector();self.sel.register(self.p.stdout,selectors.EVENT_READ)
  hello=self.read();assert hello['type']=='hello' and hello['protocol']==1 and hello['sdkVersion']=='0.1.1' and hello['id']==ID
 def read(self,seconds=5):
  deadline=time.monotonic()+seconds
  while b'\n' not in self.buf:
   assert time.monotonic()<deadline,'protocol timeout'
   assert self.sel.select(max(0,deadline-time.monotonic())),'protocol timeout'
   b=os.read(self.p.stdout.fileno(),65536);assert b,'unexpected protocol EOF';self.buf+=b;assert len(self.buf)<16*1024*1024
  line,self.buf=self.buf.split(b'\n',1);assert b'\r' not in line;frame=json.loads(line);assert KEY not in line.decode(),'private key escaped protocol boundary';self.frames.append(frame);return frame
 def send(self,frame):self.p.stdin.write((json.dumps(frame,ensure_ascii=True)+'\n').encode());self.p.stdin.flush()
 def request(self,method,params):
  i='h:'+str(uuid.uuid4());self.send(dict(type='request',id=i,method=method,params=params));return i
 def wait(self,i):
  if i in self.pending:return self.pending.pop(i)
  for _ in range(300):
   f=self.read()
   if f.get('type')=='response':
    if f['id']==i:return f
    self.pending[f['id']]=f
   else: self.pending.setdefault('other',[]).append(f)
  raise AssertionError('response bound')
 def call(self,m,p):return self.wait(self.request(m,p))
 def init(self):
  q=dict(protocol=1,extensionId=ID,platform='macOS',packageDir=str(BASE),dataDir=str(self.root),tempDir=str(self.root),grantedCapabilities=['clipboard.write','shell.open'],reason='qa',migrationPaths={})
  assert self.call('initialize',q)['result']['ready']
 def open(self,s,key=KEY):return self.call('session.open',dict(sessionId=s,commandName='index',preferences=dict(apiKey=key,entriesWithoutIsoCode='show'),argumentValues={}))
 def invoke(self,s,a):return self.request('session.call',dict(sessionId=s,operation='extension:invoke-action',payload=dict(actionId=a)))
 def end(self):
  self.p.stdin.close();self.p.wait(timeout=5);assert self.p.returncode==0;assert not self.p.stderr.read();self.sel.close()
def action(v,t):return next(a['id'] for a in v['actions'] if a['title']==t)
def seed(root):
 countries=[json.loads(c['input']) for c in json.loads((BASE/'qa/oracle.json').read_text())['cases'][:2]]
 fingerprint=hashlib.sha256(str(root).encode()+b'\0'+KEY.encode()).hexdigest();d=root/'country-lookup-cache';d.mkdir(exist_ok=True)
 (d/'storage.json').write_text(json.dumps(dict(entry=dict(fingerprint=fingerprint,time=int(time.time()),countries=countries))))
with tempfile.TemporaryDirectory(dir=SCRATCH) as tmp:
 root=pathlib.Path(tmp);seed(root);p=Pipe(root);p.init()
 assert 'error' in p.open('missing',' ')
 v=p.open('one')['result'];(BASE/'qa/adapter-view.json').write_text(json.dumps(v,ensure_ascii=False,indent=2)+'\n');assert v['refreshMs'] is None
 assert 'detail' in v['root']['children'][0]['props']
 # Side effect captured, canceled and acknowledged late. SDK/adapter never replay automatically.
 call=p.invoke('one',action(v,'Copy Raw JSON'));reverse=p.read();assert reverse['method']=='host.clipboard.write';exact=reverse['params'];assert set(exact)=={'text'}
 p.send(dict(type='cancel',requestId=call,reason='qa cancellation'));assert 'error' in p.wait(call)
 p.send(dict(type='response',id=reverse['id'],result=dict(ok=True)))
 current=p.call('session.call',dict(sessionId='one',operation='extension:refresh-session',payload={}))['result'];retry=action(current,'Retry Previous Action (May Repeat)')
 call=p.invoke('one',retry)
 while True:
  f=p.read()
  if f.get('type')=='request':break
 assert f['params']==exact
 p.send(dict(type='response',id=f['id'],result=dict(ok=True,error='contradictory')))
 failed=p.wait(call)['result'];assert 'Retry Previous Action (May Repeat)' in [a['title'] for a in failed['actions']]
 call=p.invoke('one',action(failed,'Retry Previous Action (May Repeat)'));reverse=p.read();assert reverse['params']==exact;p.send(dict(type='response',id=reverse['id'],result=dict(ok=True)));done=p.wait(call)['result']
 assert 'Retry Previous Action (May Repeat)' not in [a['title'] for a in done['actions']]
 # Open uses the typed target object and retries the exact independently validated URL.
 call=p.invoke('one',action(done,'Open in Google Maps'));reverse=p.read();assert reverse['method']=='host.shell.open';url_params=reverse['params'];assert set(url_params)=={'target'}
 p.send(dict(type='response',id=reverse['id'],result=None));unconfirmed=p.wait(call)['result']
 call=p.invoke('one',action(unconfirmed,'Retry Previous Action (May Repeat)'));reverse=p.read();assert reverse['method']=='host.shell.open' and reverse['params']==url_params
 p.send(dict(type='response',id=reverse['id'],result=dict(ok=True,message='Opened')));done=p.wait(call)['result']
 stale=p.invoke('one',action(v,'Copy Country Name'));assert 'error' in p.wait(stale)
 # Latest queued queries are fenced in real SDK reader order.
 ids=[]
 for q in ['first','second','Example language']:
  ids.append(p.request('session.call',dict(sessionId='one',operation='extension:search-text-changed',payload=dict(searchText=q))))
 results=[p.wait(i) for i in ids];last=results[-1]['result'];assert last['root']['props']['searchText']=='Example language';assert len(last['root']['children'])==1
 # Own-session intent fence: second session cannot use IDs from the first.
 v2=p.open('two')['result'];cross=p.invoke('two',action(last,'Copy Country Name'));assert 'error' in p.wait(cross)
 toggle=p.invoke('one',action(last,'Toggle Details'));hidden=p.wait(toggle)['result'];assert 'detail' not in hidden['root']['children'][0]['props']
 assert json.loads((root/'country-lookup/storage.json').read_text())['showing-detail'] is False
 # Corruption blocks refresh before any public API attempt, remains visibly failed.
 (root/'country-lookup-cache/storage.json').write_text('{broken')
 failure=p.wait(p.invoke('one',action(hidden,'Refresh')))['result'];(BASE/'qa/failure-view.json').write_text(json.dumps(failure,ensure_ascii=False,indent=2)+'\n');assert failure['root']['children'][0]['props']['title']=='Could not load countries'
 again=p.wait(p.invoke('one',action(failure,'Retry')))['result'];assert again['root']['children'][0]['props']['title']=='Could not load countries'
 # Dispose, unknown IDs, EOF; no live effect is replayed.
 assert 'result' in p.call('session.dispose',dict(sessionId='one'))
 assert 'error' in p.call('session.call',dict(sessionId='one',operation='extension:refresh-session',payload={}))
 assert 'result' in p.call('session.dispose',dict(sessionId='two'))
 assert 'result' in p.call('shutdown',{})
 p.p.wait(timeout=5);assert p.p.returncode==0;p.sel.close()
 # Canceled opens must free SDK and authoritative adapter slots even without dispose.
 seed(root);p=Pipe(root);p.init()
 for n in range(140):
  i=p.request('session.open',dict(sessionId='cancel-'+str(n),commandName='index',preferences=dict(apiKey=' ',entriesWithoutIsoCode='show'),argumentValues={}))
  p.send(dict(type='cancel',requestId=i,reason='qa cancel open'));assert 'error' in p.wait(i)
 assert 'result' in p.open('after-cancels');p.end()
 # Cache is private and usable across process restart; corruption remains actionable.
 p=Pipe(root);p.init();assert 'result' in p.open('restart');p.end()
 for raw in [b'{}\r\n',b'{broken\n',b'{}',b'[]\n',b'x'*(16*1024*1024+1)+b'\n']:
  x=subprocess.Popen([str(BIN)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
  assert json.loads(x.stdout.readline())['type']=='hello'
  x.communicate(raw,timeout=5);assert x.returncode!=0
print('Production JSONL OS-pipe tests passed: hello/init/open/actions/search/cancel/late ACK/exact retry/dispose/shutdown/EOF/framing/restart/140 canceled opens. No product API or OS effect executed.')
