#!/usr/bin/env python3
"""Build/test local dependencies without replacing the committed Git lockfile."""
import os, pathlib, subprocess, sys
root=pathlib.Path(__file__).resolve().parents[1]
config=root/'.cargo/config.local.toml'
if not config.is_file(): raise SystemExit('Run python3 scripts/local-patches.py first.')
shadow=root/'.local/dev'
shadow.mkdir(parents=True,exist_ok=True)
text=(root/'Cargo.toml').read_text()
text=text.replace('path = "src/main.rs"',f'path = "{root}/src/main.rs"')
text+='\n[lib]\npath = "'+str(root/'src/lib.rs')+'"\n'
(shadow/'Cargo.toml').write_text(text)
env=os.environ.copy()
env.setdefault('CARGO_TARGET_DIR',str(root/'target'))
env.setdefault('DDI_HOME',str(root/'target/scratch/home'))
env.setdefault('PHOTOCRAFT_CONFIG_DIR',str(root/'target/scratch/photocraft'))
args=sys.argv[1:] or ['build','--release']
# Local patches intentionally have their own lockfile. Git builds use root Cargo.lock.
raise SystemExit(subprocess.call(['cargo','--config',str(config),args[0],'--manifest-path',str(shadow/'Cargo.toml'),*args[1:]],cwd=root,env=env))
