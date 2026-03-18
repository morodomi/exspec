"""Tests for generate_python_gt.py — Python observe ground truth generation."""

import pytest
from generate_python_gt import analyze_test_file


# --- GT-01: absolute import resolution ---


class TestAbsoluteImportResolution:
    def test_direct_import_becomes_primary_candidate(self, tmp_path):
        """
        Given: a test file containing `from httpx._decoders import IdentityDecoder`
        When: generate_python_gt.py analyzes it (call analyze_test_file(test_path, project_root))
        Then: `httpx/_decoders.py` is included as primary candidate with `direct_import` evidence
        """
        # Given
        project_root = tmp_path
        src_file = project_root / "httpx" / "_decoders.py"
        src_file.parent.mkdir(parents=True)
        src_file.write_text("class IdentityDecoder: pass\n")

        test_dir = project_root / "tests"
        test_dir.mkdir()
        test_file = test_dir / "test_decoders.py"
        test_file.write_text(
            "from httpx._decoders import IdentityDecoder\n\n"
            "def test_identity_decoder():\n"
            "    d = IdentityDecoder()\n"
            "    assert d is not None\n"
        )

        # When
        result = analyze_test_file(str(test_file), str(project_root))

        # Then
        assert "httpx/_decoders.py" in result["primary_targets"], (
            f"expected httpx/_decoders.py in primary_targets, got {result['primary_targets']}"
        )
        evidence = result.get("evidence", {})
        assert "direct_import" in evidence.get("httpx/_decoders.py", []), (
            f"expected direct_import evidence for httpx/_decoders.py, got {evidence}"
        )


# --- GT-02: barrel import recorded as manual annotation candidate ---


class TestBarrelImportManualAnnotation:
    def test_barrel_import_needs_manual_review(self, tmp_path):
        """
        Given: a test file containing `import httpx` and usage `httpx.Client(...)`
        When: generate_python_gt.py analyzes it
        Then: The barrel import is recorded with `barrel_import` evidence and marked for
              manual annotation (`needs_manual_review: true`)
        """
        # Given
        project_root = tmp_path
        init_file = project_root / "httpx" / "__init__.py"
        init_file.parent.mkdir(parents=True)
        init_file.write_text("from httpx._client import Client\n")

        test_dir = project_root / "tests"
        test_dir.mkdir()
        test_file = test_dir / "test_client.py"
        test_file.write_text(
            "import httpx\n\n"
            "def test_client_instantiation():\n"
            "    client = httpx.Client()\n"
            "    assert client is not None\n"
        )

        # When
        result = analyze_test_file(str(test_file), str(project_root))

        # Then
        assert result.get("needs_manual_review") is True, (
            f"expected needs_manual_review=True for barrel import, got {result.get('needs_manual_review')}"
        )
        evidence = result.get("evidence", {})
        all_evidence = [ev for evs in evidence.values() for ev in evs]
        assert "barrel_import" in all_evidence, (
            f"expected barrel_import in evidence values, got {evidence}"
        )


# --- GT-03: filename convention match ---


class TestFilenameConventionMatch:
    def test_filename_match_evidence_recorded(self, tmp_path):
        """
        Given: test file at `tests/test_utils.py` and production file `httpx/_utils.py` exists
        When: generate_python_gt.py analyzes it
        Then: `httpx/_utils.py` is included with `filename_match` evidence
        """
        # Given
        project_root = tmp_path
        utils_file = project_root / "httpx" / "_utils.py"
        utils_file.parent.mkdir(parents=True)
        utils_file.write_text("def helper(): pass\n")

        test_dir = project_root / "tests"
        test_dir.mkdir()
        test_file = test_dir / "test_utils.py"
        test_file.write_text(
            "def test_helper():\n"
            "    assert True\n"
        )

        # When
        result = analyze_test_file(str(test_file), str(project_root))

        # Then
        evidence = result.get("evidence", {})
        assert "httpx/_utils.py" in evidence, (
            f"expected httpx/_utils.py in evidence keys, got {list(evidence.keys())}"
        )
        assert "filename_match" in evidence["httpx/_utils.py"], (
            f"expected filename_match evidence for httpx/_utils.py, got {evidence['httpx/_utils.py']}"
        )
