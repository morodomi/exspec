"""Ground truth generator for Python observe dogfooding.

Uses Python ast module (NOT tree-sitter) to avoid tautological evaluation
against exspec's own observe implementation.
"""

from __future__ import annotations

import ast
from pathlib import Path
from typing import Optional

# Evidence type constants
EVIDENCE_DIRECT_IMPORT = "direct_import"
EVIDENCE_BARREL_IMPORT = "barrel_import"
EVIDENCE_FILENAME_MATCH = "filename_match"

# Test directory names excluded from production file search
TEST_DIR_NAMES = ("tests", "test", "__pycache__")

# Conventional test filename prefix
TEST_FILE_PREFIX = "test_"


def _find_production_files(project_root: str) -> list[Path]:
    """Return all .py files that are NOT in test directories."""
    root = Path(project_root)
    result = []
    for p in root.rglob("*.py"):
        parts = p.relative_to(root).parts
        if any(part in TEST_DIR_NAMES for part in parts):
            continue
        result.append(p)
    return result


def _add_evidence(
    evidence: dict[str, list[str]], rel_path: str, evidence_type: str
) -> None:
    """Add an evidence type to the evidence dict for a given path (no-op if already present)."""
    evidence.setdefault(rel_path, [])
    if evidence_type not in evidence[rel_path]:
        evidence[rel_path].append(evidence_type)


def _resolve_import_nodes(
    tree: ast.AST,
    root: Path,
    primary_targets: list[str],
    evidence: dict[str, list[str]],
) -> bool:
    """Resolve import statements from the AST.

    Processes ast.ImportFrom (direct imports) and ast.Import (plain/barrel imports).
    Returns True if any barrel import was detected (needs_manual_review).
    """
    needs_manual_review = False

    for node in ast.walk(tree):
        if isinstance(node, ast.ImportFrom):
            if node.level and node.level > 0:
                # Relative import — cannot resolve statically without package context
                needs_manual_review = True
            elif node.module:
                _resolve_from_import(node.module, root, primary_targets, evidence)

        elif isinstance(node, ast.Import):
            for alias in node.names:
                barrel_found = _resolve_plain_import(
                    alias.name, root, primary_targets, evidence
                )
                if barrel_found:
                    needs_manual_review = True

    return needs_manual_review


def _resolve_from_import(
    module: str,
    root: Path,
    primary_targets: list[str],
    evidence: dict[str, list[str]],
) -> None:
    """Resolve `from <module> import ...` to a production file candidate."""
    candidate_rel = module.replace(".", "/") + ".py"
    candidate = root / candidate_rel
    if candidate.exists() and candidate.resolve().is_relative_to(root.resolve()):
        if candidate_rel not in primary_targets:
            primary_targets.append(candidate_rel)
        _add_evidence(evidence, candidate_rel, EVIDENCE_DIRECT_IMPORT)


def _resolve_plain_import(
    module_name: str,
    root: Path,
    primary_targets: list[str],
    evidence: dict[str, list[str]],
) -> bool:
    """Resolve `import <module>` to a production file or barrel package.

    Returns True if a barrel import was detected.
    """
    # Check module file first (foo/bar.py), then package barrel (foo/bar/__init__.py)
    candidate_rel = module_name.replace(".", "/") + ".py"
    candidate = root / candidate_rel
    if candidate.exists() and candidate.resolve().is_relative_to(root.resolve()):
        if candidate_rel not in primary_targets:
            primary_targets.append(candidate_rel)
        _add_evidence(evidence, candidate_rel, EVIDENCE_DIRECT_IMPORT)
        return False

    init_candidate = root / module_name.replace(".", "/") / "__init__.py"
    if init_candidate.exists() and init_candidate.resolve().is_relative_to(root.resolve()):
        rel = str(init_candidate.relative_to(root))
        _add_evidence(evidence, rel, EVIDENCE_BARREL_IMPORT)
        return True

    return False


def _apply_filename_convention(
    test_path: Path,
    root: Path,
    primary_targets: list[str],
    secondary_targets: list[str],
    evidence: dict[str, list[str]],
    prod_files: Optional[list[Path]] = None,
) -> None:
    """Add filename convention matches to primary targets."""
    stem = test_path.stem
    bare = stem[len(TEST_FILE_PREFIX):] if stem.startswith(TEST_FILE_PREFIX) else stem

    if prod_files is None:
        prod_files = _find_production_files(str(root))

    for prod in prod_files:
        prod_stem = prod.stem
        if prod_stem == bare or prod_stem == f"_{bare}":
            rel = str(prod.relative_to(root))
            _add_evidence(evidence, rel, EVIDENCE_FILENAME_MATCH)
            if rel not in primary_targets and rel not in secondary_targets:
                primary_targets.append(rel)


def analyze_test_file(
    test_path: str, project_root: str, *, prod_files: Optional[list[Path]] = None
) -> dict:
    """Analyze a single test file and return its ground truth mapping.

    Returns:
        {
            "test_file": str,           # relative to project_root
            "primary_targets": list,    # relative paths
            "secondary_targets": list,
            "needs_manual_review": bool,
            "evidence": {path: [evidence_type, ...]},
        }
    """
    root = Path(project_root)
    test_p = Path(test_path)
    rel_test = str(test_p.relative_to(root))

    primary_targets: list[str] = []
    secondary_targets: list[str] = []
    needs_manual_review = False
    evidence: dict[str, list[str]] = {}

    source = test_p.read_text(encoding="utf-8")
    try:
        tree = ast.parse(source, filename=test_path)
    except SyntaxError:
        return {
            "test_file": rel_test,
            "primary_targets": primary_targets,
            "secondary_targets": secondary_targets,
            "needs_manual_review": True,
            "evidence": evidence,
        }

    needs_manual_review = _resolve_import_nodes(tree, root, primary_targets, evidence)
    _apply_filename_convention(
        test_p, root, primary_targets, secondary_targets, evidence, prod_files=prod_files
    )

    return {
        "test_file": rel_test,
        "primary_targets": primary_targets,
        "secondary_targets": secondary_targets,
        "needs_manual_review": needs_manual_review,
        "evidence": evidence,
    }


def generate_ground_truth(project_root: str, test_dir: str) -> dict:
    """Generate ground truth for all test files in test_dir.

    Returns a dict compatible with evaluate_observe.py:
        {"file_mappings": {test_file: {primary_targets, secondary_targets, evidence}}}
    """
    root = Path(project_root)
    test_root = Path(test_dir)

    prod_files = _find_production_files(project_root)

    file_mappings: dict[str, dict] = {}
    for test_file in sorted(test_root.rglob("test_*.py")):
        result = analyze_test_file(str(test_file), str(root), prod_files=prod_files)
        rel = result["test_file"]
        file_mappings[rel] = {
            "primary_targets": result["primary_targets"],
            "secondary_targets": result["secondary_targets"],
            "needs_manual_review": result["needs_manual_review"],
            "evidence": result["evidence"],
        }

    return {"file_mappings": file_mappings}
