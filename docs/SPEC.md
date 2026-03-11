# exspec Specification

各ルールの入力テストコード→期待出力を定義する。fixtures/ 作成とテスト設計の根拠文書。

## Notation

- `BLOCK` / `WARN` / `INFO` / `PASS`: 判定レベル
- 入力例は Python / TypeScript を中心に記載（PHP / Rust は Language セクション参照）
- 閾値はデフォルト値。`.exspec.toml` で変更可能

---

## Tier 1 Rules

### T001: assertion-free

テスト関数内にアサーション（assert/expect/should等）が1つもない。

**Default**: BLOCK

#### Python -- Violation

```python
# fixtures/python/t001_violation.py
def test_create_user():
    user = create_user("alice")
    # No assertion -- just calling the function
```

#### Python -- Pass

```python
# fixtures/python/t001_pass.py
def test_create_user():
    user = create_user("alice")
    assert user.name == "alice"
```

#### TypeScript -- Violation

```typescript
// fixtures/typescript/t001_violation.test.ts
test('create user', () => {
  const user = createUser('alice');
  // No assertion
});
```

#### TypeScript -- Pass

```typescript
// fixtures/typescript/t001_pass.test.ts
test('create user', () => {
  const user = createUser('alice');
  expect(user.name).toBe('alice');
});
```

#### Expected Output

```
BLOCK tests/test_api.py:1  T001 assertion-free
```

#### Detection

- tree-sitter query: テスト関数のbody内に `assert`, `assertEqual`, `assertRaises`, `expect(`, `.should`, `.toBe`, `.toEqual` 等が存在しない
- scm: `assertion.scm` のマッチが0件

**Python assertion patterns** (assertion.scm):
- `assert` statement
- `self.assert*()` (unittest)
- `pytest.raises()` — exception verification
- `mock.assert_*()` (unittest.mock)
- `pytest.warns()` — warning verification
- `pytest.fail()` — explicit failure oracle (unconditionally fails the test)

---

### T002: mock-overuse

テスト関数内のmock/stub/spy数が閾値を超過、または異なるクラス/モジュールのmock数が閾値を超過。

**Default**: WARN
**Thresholds**: `mock_max=5`, `mock_class_max=3`

#### Python -- Violation

```python
# fixtures/python/t002_violation.py
from unittest.mock import patch, MagicMock

def test_process_order():
    mock_db = MagicMock()
    mock_payment = MagicMock()
    mock_email = MagicMock()
    mock_inventory = MagicMock()
    mock_logger = MagicMock()
    mock_cache = MagicMock()
    # 6 mocks across 6 different classes
    result = process_order(mock_db, mock_payment, mock_email,
                           mock_inventory, mock_logger, mock_cache)
    assert result.success
```

#### Python -- Pass

```python
# fixtures/python/t002_pass.py
from unittest.mock import MagicMock

def test_process_order():
    mock_db = MagicMock()
    result = process_order(mock_db)
    assert result.success
```

#### TypeScript -- Violation

```typescript
// fixtures/typescript/t002_violation.test.ts
test('process order', () => {
  const mockDb = jest.fn();
  const mockPayment = jest.fn();
  const mockEmail = jest.fn();
  const mockInventory = jest.fn();
  const mockLogger = jest.fn();
  const mockCache = jest.fn();
  const result = processOrder(mockDb, mockPayment, mockEmail,
                               mockInventory, mockLogger, mockCache);
  expect(result.success).toBe(true);
});
```

#### TypeScript -- Pass

```typescript
// fixtures/typescript/t002_pass.test.ts
test('process order', () => {
  const mockDb = jest.fn();
  const result = processOrder(mockDb);
  expect(result.success).toBe(true);
});
```

#### Expected Output

```
WARN tests/test_order.py:4  T002 mock-overuse (6 mocks across 6 classes)
```

#### Detection

- tree-sitter query: `mock_usage.scm` -- `MagicMock()`, `Mock()`, `patch(`, `jest.fn()`, `jest.mock(`, `vi.fn()`, `sinon.stub(`, `sinon.spy(` 等のカウント
- 異クラス判定: mock変数名からクラス/モジュール名を推定（`mock_db` → db, `mockPayment` → Payment）

---

### T003: giant-test

テスト関数の行数が閾値を超過。

**Default**: WARN
**Threshold**: `test_max_lines=50`

#### Python -- Violation

```python
# fixtures/python/t003_violation.py
def test_full_workflow():
    # ... 73 lines of setup, action, and assertions ...
    user = create_user("alice")
    # (50+ lines of code)
    assert result.final_status == "complete"
```

#### Python -- Pass

```python
# fixtures/python/t003_pass.py
def test_create_user():
    user = create_user("alice")
    assert user.name == "alice"
    assert user.active is True
```

#### TypeScript -- Violation

```typescript
// fixtures/typescript/t003_violation.test.ts
test('full workflow', () => {
  // ... 73 lines ...
  expect(result.finalStatus).toBe('complete');
});
```

#### Expected Output

```
WARN tests/test_workflow.py:1  T003 giant-test (73 lines)
```

#### Detection

- tree-sitter query: テスト関数ノードの `start_point.row` と `end_point.row` の差分

---

### T004: no-parameterized

ファイル内のパラメタライズドテスト比率が閾値未満。

**Default**: INFO
**Threshold**: `parameterized_min_ratio=0.1`

#### Python -- Violation

```python
# fixtures/python/t004_violation.py
def test_validate_email_valid():
    assert validate_email("user@example.com") is True

def test_validate_email_invalid():
    assert validate_email("invalid") is False

def test_validate_email_empty():
    assert validate_email("") is False
```

#### Python -- Pass

```python
# fixtures/python/t004_pass.py
import pytest

@pytest.mark.parametrize("email,expected", [
    ("user@example.com", True),
    ("invalid", False),
    ("", False),
])
def test_validate_email(email, expected):
    assert validate_email(email) is expected
```

#### TypeScript -- Pass

```typescript
// fixtures/typescript/t004_pass.test.ts
test.each([
  ['user@example.com', true],
  ['invalid', false],
  ['', false],
])('validate email %s', (email, expected) => {
  expect(validateEmail(email)).toBe(expected);
});
```

#### Expected Output (violation)

```
INFO tests/test_email.py  T004 no-parameterized (0% parameterized, threshold: 10%)
```

#### Detection

- Python: `@pytest.mark.parametrize` デコレータ
- TypeScript: `test.each`, `it.each`, `describe.each`
- scm: `parameterized.scm`

---

### T005: pbt-missing

ファイル/プロジェクト内にProperty-Based Testingライブラリのimportがない。

**Default**: INFO

#### Python -- Violation

```python
# fixtures/python/t005_violation.py
# No hypothesis import anywhere
def test_sort():
    assert sort_list([3, 1, 2]) == [1, 2, 3]
```

#### Python -- Pass

```python
# fixtures/python/t005_pass.py
from hypothesis import given
from hypothesis import strategies as st

@given(st.lists(st.integers()))
def test_sort_idempotent(xs):
    assert sort_list(sort_list(xs)) == sort_list(xs)
```

#### TypeScript -- Pass

```typescript
// fixtures/typescript/t005_pass.test.ts
import fc from 'fast-check';

test('sort is idempotent', () => {
  fc.assert(fc.property(fc.array(fc.integer()), (xs) => {
    expect(sortList(sortList(xs))).toEqual(sortList(xs));
  }));
});
```

#### Expected Output (violation)

```
INFO tests/test_sort.py  T005 pbt-missing
```

#### Detection

- Python: `hypothesis`, `schemathesis` のimport
- TypeScript: `fast-check`, `jsverify` のimport

---

### T006: low-assertion-density

ファイル内のアサーション数 / テスト関数数 < 1.0。

**Default**: WARN

#### Python -- Violation

```python
# fixtures/python/t006_violation.py
def test_create_user():
    user = create_user("alice")
    assert user is not None

def test_update_user():
    update_user("alice", name="bob")
    # No assertion

def test_delete_user():
    delete_user("alice")
    # No assertion
```

Total: 1 assertion / 3 tests = 0.33

#### Python -- Pass

```python
# fixtures/python/t006_pass.py
def test_create_user():
    user = create_user("alice")
    assert user.name == "alice"
    assert user.active is True

def test_update_user():
    update_user("alice", name="bob")
    user = get_user("alice")
    assert user.name == "bob"
```

Total: 3 assertions / 2 tests = 1.5

#### Expected Output (violation)

```
WARN tests/test_user.py  T006 low-assertion-density (0.33 assertions/test)
```

#### Detection

- `assertion.scm` のマッチ数 / `test_function.scm` のマッチ数

---

### T007: test-source-ratio

テストファイル数 / ソースファイル数の比率を報告。ルール違反ではなくメトリクスとして出力。

**Default**: INFO

#### Expected Output

```
INFO  T007 test-source-ratio (tests: 12, sources: 45, ratio: 0.27)
```

#### Detection

- `[paths].test_patterns` に一致するファイル数 / それ以外のソースファイル数
- プロジェクト全体のメトリクス（ファイル単位ではない）

---

### T008: no-contract

テストコード内でContract/Schema検証ライブラリ（Pydantic, Zod, Pandera等）が使用されていない。

**Default**: INFO

#### Python -- Violation

```python
# fixtures/python/t008_violation.py
def test_api_response():
    response = get_api_response()
    assert response["name"] == "alice"
    assert response["age"] == 30
    # Manual field-by-field check instead of schema validation
```

#### Python -- Pass

```python
# fixtures/python/t008_pass.py
from pydantic import BaseModel

class UserResponse(BaseModel):
    name: str
    age: int

def test_api_response():
    response = get_api_response()
    user = UserResponse(**response)  # Schema validation
    assert user.name == "alice"
```

#### TypeScript -- Pass

```typescript
// fixtures/typescript/t008_pass.test.ts
import { z } from 'zod';

const UserSchema = z.object({
  name: z.string(),
  age: z.number(),
});

test('api response', () => {
  const response = getApiResponse();
  const user = UserSchema.parse(response);
  expect(user.name).toBe('alice');
});
```

#### Expected Output (violation)

```
INFO tests/test_api.py  T008 no-contract (no Pydantic/Pandera)
```

#### Detection

- Python: `pydantic`, `pandera`, `marshmallow`, `attrs` のimport
- TypeScript: `zod`, `yup`, `io-ts`, `ajv` のimport
- scm: `contract.scm`

---

## Output Format Specification

### Terminal (default)

```
exspec v0.1.0 -- 42 test files, 187 test functions

BLOCK tests/test_api.py:78       T001 assertion-free
WARN  tests/test_predict.py:45   T002 mock-overuse (6 mocks across 4 classes)
WARN  tests/test_feature.py:120  T003 giant-test (73 lines)
INFO  tests/test_transform.py    T008 no-contract (no Pydantic/Pandera)

Metrics:
  Mock density:      2.3/test (avg), 4 distinct classes/test (max)
  Parameterized:     15% (28/187)
  PBT usage:         12% (23/187)
  Assertion density: 1.8/test (avg)
  Contract coverage: 8% (15/187)

Score: BLOCK 1 | WARN 2 | INFO 1 | PASS 183
```

### JSON (`--format json`)

```json
{
  "version": "0.1.0",
  "summary": {
    "files": 42,
    "functions": 187,
    "block": 1,
    "warn": 2,
    "info": 1,
    "pass": 183
  },
  "diagnostics": [
    {
      "rule": "T001",
      "level": "block",
      "file": "tests/test_api.py",
      "line": 78,
      "message": "assertion-free",
      "details": null
    }
  ],
  "metrics": {
    "mock_density_avg": 2.3,
    "mock_class_max": 4,
    "parameterized_ratio": 0.15,
    "pbt_ratio": 0.12,
    "assertion_density_avg": 1.8,
    "contract_coverage": 0.08,
    "test_source_ratio": 0.27
  }
}
```

### SARIF (`--format sarif`)

SARIF v2.1.0準拠。GitHub Code Scanning互換。

```json
{
  "$schema": "https://raw.githubusercontent.com/oasis-tcs/sarif-spec/main/sarif-2.1/schema/sarif-schema-2.1.0.json",
  "version": "2.1.0",
  "runs": [{
    "tool": {
      "driver": {
        "name": "exspec",
        "version": "0.1.0",
        "rules": [
          {
            "id": "T001",
            "name": "assertion-free",
            "shortDescription": { "text": "Test function has no assertions" },
            "defaultConfiguration": { "level": "error" }
          }
        ]
      }
    },
    "results": [
      {
        "ruleId": "T001",
        "level": "error",
        "message": { "text": "assertion-free" },
        "locations": [{
          "physicalLocation": {
            "artifactLocation": { "uri": "tests/test_api.py" },
            "region": { "startLine": 78 }
          }
        }]
      }
    ]
  }]
}
```

SARIF level mapping: BLOCK=error, WARN=warning, INFO=note

### AI Prompt (`--format ai-prompt`)

```markdown
## AI Test Quality Review Request

以下のテストファイルについて、意味論的な品質チェックを実施してください。

### tests/test_predict.py
- 6つのmockが4つの異なるクラスに対して使用されています
- 質問: これらのmockは外部依存の分離のためですか、それとも内部実装の検証ですか？
- 質問: mockの振る舞いは実際のサービスの仕様と一致していますか？

### tests/test_feature.py
- 73行のテスト関数があります
- 質問: このテストは複数の独立した仕様を1関数にまとめていませんか？
- 質問: Given/When/Then の構造で分割可能ですか？
```

---

## Config Specification (.exspec.toml)

```toml
[general]
lang = ["python", "typescript"]    # 解析対象言語

[rules]
disable = ["T004", "T005"]         # 無効化するルールID

[thresholds]
mock_max = 5                       # T002: 1テスト関数内のmock数上限
mock_class_max = 3                 # T002: 異なるクラスのmock数上限
test_max_lines = 50                # T003: テスト関数行数上限
parameterized_min_ratio = 0.1      # T004: パラメタライズドテスト比率下限

[paths]
test_patterns = [                  # テストファイルのglobパターン
  "tests/**",
  "**/*_test.*",
  "**/*.test.*",
  "**/*.spec.*"
]
ignore = [                         # 除外パターン
  "node_modules",
  ".venv",
  "vendor",
  "target",
  "dist"
]
```

### Config Resolution Order

1. CLI引数 (最優先)
2. `.exspec.toml` (プロジェクトルート)
3. デフォルト値

---

## Inline Suppression Specification

### Syntax

```
# exspec-ignore: <RULE_ID>[, <RULE_ID>...]
```

### Scope

- **Line-level**: コメントの次の行に適用
- **Function-level**: テスト関数定義の直前に配置

### Examples

```python
# exspec-ignore: T002
def test_complex_integration():
    # This function intentionally uses many mocks
    ...

# exspec-ignore: T002, T003
def test_full_e2e():
    ...
```

```typescript
// exspec-ignore: T002
test('complex integration', () => {
  ...
});
```

### Limitations

- Suppression applies only to the **immediate next test function** (`test()`, `it()`, `def test_*`).
- Placing `// exspec-ignore:` above `describe()` does **not** propagate to inner `test()`/`it()` calls.
- To suppress a rule for multiple tests, add the comment above each individual test function.
- This applies equally to Python (above `def test_*`) and PHP (above `public function test*`).

```typescript
// This does NOT suppress T001 for inner tests:
// exspec-ignore: T001
describe('user management', () => {
  test('create user', () => { ... });  // T001 still fires
  test('delete user', () => { ... });  // T001 still fires
});

// This DOES suppress T001:
// exspec-ignore: T001
test('create user', () => { ... });  // T001 suppressed
```

### Detection

- コメントノードから `exspec-ignore:` パターンをパース
- 次のテスト関数ノードに対してルールを除外

---

## Language: Rust (cargo test)

Phase 5A で追加。tree-sitter-rust 0.23 (ABI 14) による静的解析。

### Test File Detection

| Pattern | Example |
|---------|---------|
| `tests/**/*.rs` | `tests/integration_test.rs` |
| `*_test.rs` | `user_service_test.rs` |

### Known Limitations (MVP)

- `#[cfg(test)] mod tests {}` inline tests in `src/` files are **not** detected (files not recognized as test files by path pattern)
- Only `tests/` directory and `*_test.rs` pattern files are analyzed
- Helper modules in `tests/` (e.g., `tests/common/mod.rs`) are scanned but produce no results if they contain no `#[test]` functions

### Test Function Detection

| Pattern | Example |
|---------|---------|
| `#[test]` | `#[test] fn test_example() {}` |
| `#[tokio::test]` | `#[tokio::test] async fn test_async() {}` |
| `#[async_std::test]` | `#[async_std::test] async fn test_async() {}` |

tree-sitter AST note: `attribute_item` and `function_item` are sibling nodes (not parent-child). Detection uses `attribute_item` capture + `next_sibling()` walk.

### Rule Mapping

| Rule | Rust Pattern | Notes |
|------|-------------|-------|
| T001 assertion-free | `assert!`, `assert_eq!`, `assert_ne!`, `debug_assert!` | Macro invocations |
| T002 mock-overuse | `MockXxx::new()` (mockall crate) | `let mock_xxx = MockXxx::new()` for class names |
| T003 giant-test | Line count of `fn` body | Same threshold (50 lines) |
| T004 no-parameterized | `#[rstest]` attribute (rstest crate) | |
| T005 pbt-missing | `use proptest` / `use quickcheck` | |
| T006 low-assertion-density | Total assertions / total functions | |
| T007 test-source-ratio | `.rs` file counts | |
| T008 no-contract | N/A | Always INFO (no standard Rust validation crate) |

### Inline Suppression

```rust
// exspec-ignore: T001
#[test]
fn test_suppressed() {
    // T001 suppressed
}
```

Comment must be on the line immediately before `#[test]` attribute.

---

## Tier 2 Rules

### T101: how-not-what

テストが「何を検証しているか (WHAT)」ではなく「どう実装されているか (HOW)」を検証しているパターンを検出する。以下の2種類のパターンを検出しWARNを出力する:

1. **Mock検証メソッド**: `assert_called_with()`, `toHaveBeenCalledWith()` 等
2. **Private attribute access in assertions**: assertion内での `obj._private` アクセス (#13)

**Default**: WARN
**Threshold**: なし (1つ以上でWARN)

#### Assertion二重計上 (意図的仕様)

`assert_called_with()` や `toHaveBeenCalledWith()` は assertion.scm にもマッチするため、T001 (assertion-free) を回避しつつ T101 が発火する。これは意図的な設計: mock検証しかないテストは「assertionはあるが実装検証」という正しい判定になる。

#### Python -- Violation

```python
# fixtures/python/t101_violation.py
def test_user_creation_calls_repository(mock_repo):
    service = UserService(mock_repo)
    service.create_user("alice")
    mock_repo.save.assert_called_with("alice")
    mock_repo.save.assert_called_once()
    assert service is not None
```

#### Python -- Pass

```python
# fixtures/python/t101_pass.py
def test_user_creation_returns_user():
    service = UserService()
    user = service.create_user("alice")
    assert user.name == "alice"
    assert user.active is True
```

#### TypeScript -- Violation

```typescript
// fixtures/typescript/t101_violation.test.ts
test('calls repository on create', () => {
  const mockRepo = jest.fn();
  const service = new UserService(mockRepo);
  service.createUser('alice');
  expect(mockRepo.save).toHaveBeenCalledWith('alice');
  expect(mockRepo.save).toHaveBeenCalledTimes(1);
});
```

#### TypeScript -- Pass

```typescript
// fixtures/typescript/t101_pass.test.ts
test('creates user with correct name', () => {
  const service = new UserService();
  const user = service.createUser('alice');
  expect(user.name).toBe('alice');
  expect(user.active).toBe(true);
});
```

#### Expected Output

```
WARN tests/test_api.py:1  T101 how-not-what: 2 implementation-testing pattern(s) detected
```

#### Detection

**Mock verification patterns** (how_not_what.scm の `@how_pattern`):
- Python: `mock.assert_called()`, `mock.assert_called_with()`, `mock.assert_called_once()`, `mock.assert_called_once_with()`, `mock.assert_any_call()`, `mock.assert_has_calls()`, `mock.assert_not_called()`
- TypeScript: `toHaveBeenCalled()`, `toHaveBeenCalledWith()`, `toHaveBeenCalledTimes()`, `toHaveBeenLastCalledWith()`, `toHaveBeenNthCalledWith()`, `toHaveReturned()`, `toHaveReturnedWith()`, `toHaveReturnedTimes()`, `toHaveLastReturnedWith()`, `toHaveNthReturnedWith()`

**Private attribute access in assertions** (private_in_assertion.scm の `@private_access`, assertion.scm の `@assertion` 範囲内限定):
- Python: `assert obj._private == x` (assertion文内の `obj._attr` アクセス)
- TypeScript: `expect(obj._private)` (dot notation) / `expect(obj['_private'])` (bracket notation)
- `__dunder__` は除外 (`^_[^_]` パターン)
- assertion外の `_private` アクセスは検出しない (false positive 防止)

**スコープ外**:
- PHP/Rust: how_not_what_count = 0 固定
- 2パス方式: `count_captures_within_context()` (core/query_utils.rs)

### T102: fixture-sprawl

テスト関数が依存するフィクスチャ/セットアップ変数が閾値を超えている。

**Default**: WARN
**Threshold**: `fixture_max = 5` (configurable)

#### Python -- Violation

```python
# fixtures/python/t102_violation.py
def test_complex(db, cache, queue, mailer, logger, config):
    result = process(db, cache, queue, mailer, logger, config)
    assert result.success
```

#### Python -- Pass

```python
# fixtures/python/t102_pass.py
def test_simple(db, config):
    result = process(db, config)
    assert result.success
```

#### TypeScript -- Violation

```typescript
// fixtures/typescript/t102_violation.test.ts
describe('complex', () => {
  const db = createDb();
  const cache = createCache();
  const queue = createQueue();
  const mailer = createMailer();
  const logger = createLogger();
  const config = createConfig();

  test('processes data', () => {
    expect(process(db)).toBeTruthy();
  });
});
```

#### Detection

- Python: テスト関数のパラメータ数 (`self`/`cls` 除外)
- TypeScript: テスト関数を囲む `describe` ブロック内の直接の変数宣言数 (ネストされた `describe` は累積)
- PHP/Rust: fixture_count = 0 固定 (deferred)

### T103: missing-error-test

テストファイル内に異常系 (エラーケース) のテストが1つもない。

**Default**: INFO

#### Python -- Violation

```python
# fixtures/python/t103_violation.py
def test_add():
    assert add(1, 2) == 3

def test_subtract():
    assert subtract(5, 3) == 2
```

#### Python -- Pass

```python
# fixtures/python/t103_pass.py
def test_add():
    assert add(1, 2) == 3

def test_invalid_input():
    with pytest.raises(ValueError):
        add("a", "b")
```

#### Detection

`error_test.scm` の `@error_test` で以下をファイル全体スキャン (`has_any_match` パターン):

- Python: `pytest.raises`, `self.assertRaises`, `self.assertRaisesRegex`, `self.assertWarns`, `self.assertWarnsRegex`
- TypeScript: `.toThrow()`, `.toThrowError()`, `.rejects`
- PHP/Rust: has_error_test = true 固定 (deferred)

### T104: hardcoded-only

テスト関数内のアサーションが全てハードコードリテラルのみで、変数参照がない。

**Default**: INFO
**Threshold**: なし (binary)

#### Python -- Violation

```python
# fixtures/python/t104_violation.py
def test_add():
    assert add(1, 2) == 3
    assert add(0, 0) == 0
```

#### Python -- Pass

```python
# fixtures/python/t104_pass_variable.py
def test_add():
    result = add(1, 2)
    assert result == 3
```

```python
# fixtures/python/t104_pass_computed.py
def test_roundtrip():
    data = b"hello"
    assert decode(encode(data)) == data
```

#### TypeScript -- Violation

```typescript
// fixtures/typescript/t104_violation.test.ts
test('add', () => {
  expect(add(1, 2)).toBe(3);
  expect(add(0, 0)).toBe(0);
});
```

#### TypeScript -- Pass

```typescript
// fixtures/typescript/t104_pass_variable.test.ts
test('add', () => {
  const result = add(1, 2);
  expect(result).toBe(3);
});
```

#### Detection

**Node API** (not .scm query) -- callee除外ロジックがquery predicatesでは表現困難なため、Rustで直接ASTウォーク。

`has_non_callee_identifier()` で assertion ノード内を再帰ウォーク:
1. `identifier` → builtin定数 (Python: True/False/None, TS: undefined/null) でなければ非hardcoded
2. `call` → `function` フィールドはスキップ (callee)、`arguments` のみ再帰
3. TypeScript: `has_non_callee_identifier_in_callee_chain()` で `expect(x).toBe(y)` の callee chain を辿りながら各 level の arguments をチェック

**スコープ外**: PHP/Rust: hardcoded_only = false 固定 (deferred)

### T105: deterministic-no-metamorphic

ファイル内の全アサーションが完全一致 (equality) のみで、関係演算 (>, <, in, contains 等) がない。

**Default**: INFO
**Threshold**: `min_assertions_for_t105 = 5` (configurable)

#### Python -- Violation

```python
# fixtures/python/t105_violation.py
def test_a():
    assert add(1, 2) == 3

def test_b():
    assert add(0, 0) == 0

# ... (5+ assertions, all ==)
```

#### Python -- Pass

```python
# fixtures/python/t105_pass_relational.py
def test_positive():
    assert add(1, 2) > 0

def test_contains():
    assert "hello" in greet("world")
```

#### TypeScript -- Violation

```typescript
// fixtures/typescript/t105_violation.test.ts
test('a', () => { expect(add(1,2)).toBe(3); });
test('b', () => { expect(add(0,0)).toBe(0); });
// ... (5+ assertions, all toBe/toEqual)
```

#### TypeScript -- Pass

```typescript
// fixtures/typescript/t105_pass_relational.test.ts
test('positive', () => { expect(add(1,2)).toBeGreaterThan(0); });
```

#### Detection

`relational_assertion.scm` の `@relational` でファイル全体スキャン (`has_any_match` パターン):

**Python relational patterns** (存在すれば T105 を抑制):
- comparison operators: `>`, `<`, `>=`, `<=`, `in`, `not in`, `is`, `is not`
- unittest: `assertGreater`, `assertLess`, `assertIn`, `assertIsInstance`, `assertAlmostEqual`, `assertRegex`, `assertIsNone`, `assertTrue`, `assertFalse` 等
- `pytest.approx()`

**TypeScript relational patterns**:
- `toBeGreaterThan`, `toBeLessThan`, `toContain`, `toMatch`, `toBeInstanceOf`, `toBeCloseTo`, `toHaveLength`
- `toBeNull`, `toBeUndefined`, `toBeDefined`, `toBeTruthy`, `toBeFalsy`, `toBeNaN`

**スコープ外**: PHP/Rust: has_relational_assertion = true 固定 (deferred)
