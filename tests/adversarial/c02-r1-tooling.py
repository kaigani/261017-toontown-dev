"""Permanent C0.2 regression tests. Run: python tests/adversarial/c02-r1-tooling.py"""
import contextlib
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('tablet_analysis', ROOT/'scripts/analyse-tablet-log.py')
analysis = importlib.util.module_from_spec(spec)
spec.loader.exec_module(analysis)

class ToolingTests(unittest.TestCase):
    def test_C02_R1_08_macos_accessor_contracts(self):
        prelude = (ROOT/'tests/adversarial/c02-r1-macos-accessors.rs').read_text()
        source = (ROOT/'desktop/src-tauri/src/spikes/tablet/native_macos.rs').read_text()
        conversion = source[source.index('fn device_name('):]
        with tempfile.TemporaryDirectory(prefix='toontown-c02-mac-') as tmp:
            path=Path(tmp)/'mac_test.rs'; path.write_text(prelude+'\n'+conversion)
            exe=Path(tmp)/('mac_test.exe' if sys.platform=='win32' else 'mac_test')
            built=subprocess.run(['rustc','--edition=2024','--test',str(path),'-o',str(exe)],capture_output=True,text=True)
            self.assertEqual(built.returncode,0,built.stdout+built.stderr)
            result=subprocess.run([str(exe),'--nocapture'],capture_output=True,text=True)
            self.assertEqual(result.returncode,0,result.stdout+result.stderr)

    def test_C02_R1_02_native_timestamp_association(self):
        prelude = (ROOT/'tests/adversarial/c02-r1-native-timestamps.rs').read_text()
        source = (ROOT/'desktop/src-tauri/src/spikes/tablet/native_windows.rs').read_text()
        # Compile the unchanged production pump, not a second implementation of its logic.
        pump = source[source.index('fn pump('):]
        with tempfile.TemporaryDirectory(prefix='toontown-c02-r1-') as tmp:
            path = Path(tmp)/'native_test.rs'; path.write_text(prelude+'\n'+pump)
            exe = Path(tmp)/('native_test.exe' if sys.platform=='win32' else 'native_test')
            built = subprocess.run(['rustc','--edition=2024','--test',str(path),'-o',str(exe)],capture_output=True,text=True)
            self.assertEqual(built.returncode,0,built.stdout+built.stderr)
            result = subprocess.run([str(exe),'--nocapture'],capture_output=True,text=True)
            self.assertEqual(result.returncode,0,result.stdout+result.stderr)

    def test_C02_R1_03_native_pose_only_contact_boundaries(self):
        # Valid AppKit tablet-point input can arrive without mouse-down/up events.
        packets=[]
        for start in [0,1000]:
            for dt,pressure in [(0,0),(4,.5),(8,.5),(12,.5),(16,0)]:
                packets.append(dict(kind='pose',deviceMs=start+dt,hostEpochMs=1000000+start+dt,
                                    pressure=pressure,tiltX=0,tiltY=0,twist=0,tool='pen',x=0,y=0))
        log=dict(environment=dict(userAgent='fixture',devicePixelRatio=2,coalescedApi=False),
                 summary=dict(nativeInfo=None,invalidNativeBatches=0),web=dict(samples=[]),native=dict(packets=packets))
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'log.json'; path.write_text(json.dumps(log))
            result=subprocess.run([sys.executable,str(ROOT/'scripts/analyse-tablet-log.py'),str(path)],capture_output=True,text=True)
        self.assertEqual(result.returncode,0,result.stderr)
        self.assertIn('native path: 2 strokes',result.stdout)

    def test_C02_R1_07_nearest_rank_percentile_matches_frontend(self):
        # Nearest-rank p50 of six dispatches = third observation, not fourth.
        self.assertEqual(analysis.pct([1,2,3,4,5,6],50),3)

if __name__ == '__main__': unittest.main(verbosity=2)
