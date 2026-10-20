"""Release trust must be checked before running an untrusted executable."""
import importlib.util
import pathlib
import subprocess
import sys
import unittest
from unittest.mock import patch

ROOT = pathlib.Path(__file__).resolve().parent.parent
SPEC = importlib.util.spec_from_file_location('release_package', ROOT / 'scripts/package.py')
PACKAGE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PACKAGE)


class ReleaseTrustTests(unittest.TestCase):
    def test_signature_failure_stops_before_binary_execution_or_output(self):
        with patch.object(sys, 'argv', ['package.py', '--runtime-notices', 'unused']), \
                patch.object(PACKAGE, 'verify_signatures', side_effect=ValueError('untrusted')) as verify, \
                patch.object(PACKAGE.subprocess, 'run') as run, \
                patch.object(pathlib.Path, 'mkdir') as mkdir:
            with self.assertRaisesRegex(ValueError, 'untrusted'):
                PACKAGE.main()
            self.assertEqual([p.name for p in verify.call_args.args[1]],
                             ['mcc.exe', 'mcc-ui.exe', 'Install.ps1', 'Uninstall.ps1'])
            run.assert_not_called()
            mkdir.assert_not_called()

    def test_verifier_failure_propagates(self):
        failure = subprocess.CalledProcessError(1, 'powershell.exe')
        with patch.object(sys, 'platform', 'win32'), \
                patch.object(PACKAGE.subprocess, 'run', side_effect=failure) as run:
            with self.assertRaises(subprocess.CalledProcessError):
                PACKAGE.verify_signatures(ROOT, [ROOT / 'target/release/mcc.exe'])
            self.assertTrue(run.call_args.kwargs['check'])
            self.assertIn(str((ROOT / 'target/release/mcc.exe').resolve()), run.call_args.args[0])

    def test_non_windows_cannot_claim_authenticode_verification(self):
        with patch.object(sys, 'platform', 'linux'), patch.object(PACKAGE.subprocess, 'run') as run:
            with self.assertRaisesRegex(ValueError, 'requires Windows'):
                PACKAGE.verify_signatures(ROOT, [])
            run.assert_not_called()


if __name__ == '__main__':
    unittest.main()
