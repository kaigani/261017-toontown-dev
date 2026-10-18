"""Negative controls for the dependency proof; no GUI or GPU needs to be loaded."""
import importlib.util
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('guard', ROOT / 'scripts/check-headless-deps.py')
guard = importlib.util.module_from_spec(spec)
spec.loader.exec_module(guard)


class DependencyProof(unittest.TestCase):
    def test_known_forbidden_controls(self):
        with patch.object(sys, 'platform', 'win32'):
            for dll in ['opengl32.dll', 'd3d11.dll', 'dxgi.dll', 'user32.dll', 'gdi32.dll', 'shell32.dll']:
                self.assertEqual(guard.binary_violations([dll]), [dll])
        with patch.object(sys, 'platform', 'darwin'):
            for name in ['Cocoa', 'AppKit', 'Metal', 'QuartzCore']:
                lib = f'/System/Library/Frameworks/{name}.framework/Versions/A/{name}'
                self.assertEqual(guard.binary_violations([lib]), [lib])

    def test_direct_graphics_imports_cannot_pass(self):
        with patch.object(sys, 'platform', 'win32'):
            for dll in ['dcomp.dll', 'd3d10_1.dll', 'win32u.dll']:
                with self.subTest(dll=dll):
                    self.assertEqual(guard.binary_violations([dll]), [dll], 'C01-R1-06')

    def test_gui_and_gpu_crates_cannot_pass(self):
        for crate in ['sdl2', 'sdl2-sys', 'ash']:
            with self.subTest(crate=crate):
                self.assertIsNotNone(guard.FORBIDDEN_CRATES.match(crate), 'C01-R1-06')


if __name__ == '__main__':
    unittest.main()
