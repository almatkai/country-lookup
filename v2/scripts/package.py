#!/usr/bin/env python3
"""Deterministic, offline, development-only schema-3 ZIP. Never installs/releases."""
from pathlib import Path
import json,hashlib,zipfile,stat,subprocess,tempfile,sys,os
B=Path(__file__).resolve().parents[1];binary=Path(sys.argv[1]).resolve() if len(sys.argv)>1 else B/'target/release/country-lookup-native'
assert binary.is_file();data=binary.read_bytes();assert data[:4]==bytes.fromhex('cffaedfe');assert int.from_bytes(data[4:8],'little')==0x0100000c,'Expected native ARM64 Mach-O'
sha=lambda b:hashlib.sha256(b).hexdigest()
icon=(B/'resources/extension_iconv1.png').read_bytes();assert len(icon)==191447 and sha(icon)=='f5e19ace390331fd21d581a1a512719462c1eea4635164452ca63ae8b855f55f'
files={'bin/country-lookup-native':data,'resources/extension_iconv1.png':icon,'LICENSE':(B/'LICENSE').read_bytes(),'NOTICE':(B/'NOTICE').read_bytes(),'resources/dependency-grants.json':(B/'reports/dependency-grants.json').read_bytes()}
for f in sorted((B/'licenses').rglob('*')):
 if f.is_file():assert not f.is_symlink();files['resources/licenses/'+str(f.relative_to(B/'licenses'))]=f.read_bytes()
manifest=json.loads((B/'extension.json').read_text());assert manifest['artifacts']=={};manifest['files']={f:sha(b) for f,b in sorted(files.items())};files['extension.json']=(json.dumps(manifest,ensure_ascii=False,indent=2)+'\n').encode()
dist=B/'dist';dist.mkdir(exist_ok=True);archives=[]
for name in ['country-lookup-aarch64-apple-darwin-development.zip','repeat-development.zip']:
 p=dist/name
 with zipfile.ZipFile(p,'w',compression=zipfile.ZIP_STORED) as z:
  for f,b in sorted(files.items()):
   i=zipfile.ZipInfo(f,date_time=(1980,1,1,0,0,0));i.create_system=3;i.external_attr=((stat.S_IFREG|(0o755 if f.startswith('bin/') else 0o644))<<16);i.compress_type=zipfile.ZIP_STORED;z.writestr(i,b)
 archives.append(p)
assert archives[0].read_bytes()==archives[1].read_bytes()
with tempfile.TemporaryDirectory(dir=B/'qa/scratch') as directory:
 root=Path(directory)
 with zipfile.ZipFile(archives[0]) as z:
  assert set(z.namelist())==set(files)
  for f in z.namelist():assert not f.startswith('/') and '..' not in Path(f).parts
  z.extractall(root)
 for f,b in files.items():assert (root/f).read_bytes()==b
 exe=root/'bin/country-lookup-native';exe.chmod(0o755)
 for flag in ['--protocol-info','--self-test']:
  info=json.loads(subprocess.check_output([exe,flag]));assert info['id']==manifest['id'] and info['commands']==['index'] and info['sdkVersion']=='0.1.1'
 subprocess.run(['python3',B/'qa/protocol-smoke.py',exe],cwd=B,check=True)
report=dict(kind='development-only; artifacts empty, no install/catalog/release pins',target='aarch64-apple-darwin',nativeHeader='Mach-O 64-bit ARM64',zipBytes=archives[0].stat().st_size,zipSha256=sha(archives[0].read_bytes()),executableBytes=len(data),executableSha256=sha(data),identicalRepeatZip=True,extractedProtocolSmoke='passed; synthetic private cache only, no product API or real OS effects',inventory={f:dict(bytes=len(b),sha256=sha(b)) for f,b in sorted(files.items())})
(B/'reports/package-inventory.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({k:v for k,v in report.items() if k!='inventory'},indent=2))
