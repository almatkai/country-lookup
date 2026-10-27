#!/usr/bin/env python3
"""Copy the standalone Rust inputs to a clone-local clean directory and fetch
all dependencies, including the exact HTTPS SDK revision, using a fresh Cargo home.
Never reads the host checkout or sibling SDK. No HOME/profile variables are changed.
"""
from pathlib import Path
import os,subprocess,shutil,json,hashlib,tempfile
B=Path(__file__).resolve().parents[1];isolated=B/'qa/isolated';isolated.mkdir(exist_ok=True);D=Path(tempfile.mkdtemp(prefix='run-',dir=isolated));S=D/'source';H=D/'cargo-home';T=D/'target'
assert not S.exists(), 'Use a new clone-local isolated directory, never reuse a previous build.'
S.mkdir(parents=True);H.mkdir()
for f in ['Cargo.toml','Cargo.lock']:shutil.copyfile(B/f,S/f)
for f in ['src','.cargo']:shutil.copytree(B/f,S/f)
env=dict(os.environ,CARGO_HOME=str(H),CARGO_TARGET_DIR=str(T));log=B/'reports/isolated-build.txt'
with log.open('w') as stream:subprocess.run(['cargo','build','--locked','--release'],cwd=S,env=env,stdout=stream,stderr=subprocess.STDOUT,check=True)
binary=T/'release/country-lookup-native';assert json.loads(subprocess.check_output([binary,'--self-test']))['sdkVersion']=='0.1.1'
subprocess.run(['python3',B/'qa/protocol-smoke.py',binary],cwd=B,check=True)
(B/'reports/isolated-build.json').write_text(json.dumps(dict(sdk='https://github.com/almatkai/tezbar-extension-sdk-rust.git#e955911f0bf5b558f9d3eb5183df0adc95c22af2',freshCargoHome=True,hostOrSiblingDependency=False,binaryPath=str(binary.relative_to(B)),sourceInputs={str(f.relative_to(S)):hashlib.sha256(f.read_bytes()).hexdigest() for f in sorted(S.rglob('*')) if f.is_file()},binarySha256=hashlib.sha256(binary.read_bytes()).hexdigest(),bytes=binary.stat().st_size,protocolSmoke='passed'),indent=2)+'\n')
print('Fresh isolated HTTPS SDK build and extracted production protocol smoke passed.')
