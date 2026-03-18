# Ground Truth: Python observe — httpx

Repository: encode/httpx
Commit: b5addb64
Auditor: Human + generate_python_gt.py (auto) + manual correction
Date: 2026-03-19

## Methodology

1. `generate_python_gt.py` で自動生成 (Python `ast` による独立解析)
2. `needs_manual_review: true` のファイル (barrel import 依存) を手動監査
3. 各テストファイルの describe/assert 対象を確認し primary_targets を決定

## Key Characteristics

- Almost all tests use `import httpx` (barrel import via `__init__.py`)
- Direct imports (`from httpx._xxx import ...`) are rare (2/30 test files)
- Production files use `_` prefix (`_decoders.py`, `_client.py` etc.)
- Test subdirectories (`tests/client/`, `tests/models/`) test the same production files from different angles

## Ground Truth

```json
{
  "metadata": {
    "repository": "encode/httpx",
    "commit": "b5addb64",
    "language": "python",
    "auditor": "human+auto",
    "audit_coverage": "100%"
  },
  "file_mappings": {
    "tests/test_api.py": {
      "primary_targets": ["httpx/_api.py"],
      "secondary_targets": ["httpx/_client.py"],
      "evidence": {
        "httpx/_api.py": ["filename_match", "symbol_assertion"]
      }
    },
    "tests/test_asgi.py": {
      "primary_targets": ["httpx/_transports/asgi.py"],
      "secondary_targets": ["httpx/_client.py"],
      "evidence": {
        "httpx/_transports/asgi.py": ["filename_match", "symbol_assertion"]
      }
    },
    "tests/test_auth.py": {
      "primary_targets": ["httpx/_auth.py"],
      "secondary_targets": ["httpx/_client.py"],
      "evidence": {
        "httpx/_auth.py": ["filename_match", "symbol_assertion"]
      }
    },
    "tests/test_config.py": {
      "primary_targets": ["httpx/_config.py"],
      "secondary_targets": [],
      "evidence": {
        "httpx/_config.py": ["filename_match", "symbol_assertion"]
      }
    },
    "tests/test_content.py": {
      "primary_targets": ["httpx/_content.py"],
      "secondary_targets": [],
      "evidence": {
        "httpx/_content.py": ["filename_match", "symbol_assertion"]
      }
    },
    "tests/test_decoders.py": {
      "primary_targets": ["httpx/_decoders.py"],
      "secondary_targets": [],
      "evidence": {
        "httpx/_decoders.py": ["filename_match", "symbol_assertion"]
      }
    },
    "tests/test_exceptions.py": {
      "primary_targets": ["httpx/_exceptions.py"],
      "secondary_targets": [],
      "evidence": {
        "httpx/_exceptions.py": ["filename_match", "symbol_assertion"]
      }
    },
    "tests/test_exported_members.py": {
      "primary_targets": ["httpx/__init__.py"],
      "secondary_targets": [],
      "evidence": {
        "httpx/__init__.py": ["symbol_assertion"]
      }
    },
    "tests/test_main.py": {
      "primary_targets": ["httpx/_main.py"],
      "secondary_targets": [],
      "evidence": {
        "httpx/_main.py": ["filename_match", "symbol_assertion"]
      }
    },
    "tests/test_multipart.py": {
      "primary_targets": ["httpx/_multipart.py"],
      "secondary_targets": [],
      "evidence": {
        "httpx/_multipart.py": ["filename_match", "symbol_assertion"]
      }
    },
    "tests/test_status_codes.py": {
      "primary_targets": ["httpx/_status_codes.py"],
      "secondary_targets": [],
      "evidence": {
        "httpx/_status_codes.py": ["filename_match", "symbol_assertion"]
      }
    },
    "tests/test_timeouts.py": {
      "primary_targets": ["httpx/_config.py"],
      "secondary_targets": ["httpx/_client.py"],
      "evidence": {
        "httpx/_config.py": ["symbol_assertion"]
      }
    },
    "tests/test_utils.py": {
      "primary_targets": ["httpx/_utils.py"],
      "secondary_targets": [],
      "evidence": {
        "httpx/_utils.py": ["direct_import", "filename_match", "symbol_assertion"]
      }
    },
    "tests/test_wsgi.py": {
      "primary_targets": ["httpx/_transports/wsgi.py"],
      "secondary_targets": ["httpx/_client.py"],
      "evidence": {
        "httpx/_transports/wsgi.py": ["filename_match", "symbol_assertion"]
      }
    },
    "tests/client/test_async_client.py": {
      "primary_targets": ["httpx/_client.py"],
      "secondary_targets": [],
      "evidence": {
        "httpx/_client.py": ["symbol_assertion"]
      }
    },
    "tests/client/test_auth.py": {
      "primary_targets": ["httpx/_auth.py", "httpx/_client.py"],
      "secondary_targets": [],
      "evidence": {
        "httpx/_auth.py": ["filename_match", "symbol_assertion"],
        "httpx/_client.py": ["symbol_assertion"]
      }
    },
    "tests/client/test_client.py": {
      "primary_targets": ["httpx/_client.py"],
      "secondary_targets": [],
      "evidence": {
        "httpx/_client.py": ["filename_match", "symbol_assertion"]
      }
    },
    "tests/client/test_cookies.py": {
      "primary_targets": ["httpx/_models.py"],
      "secondary_targets": ["httpx/_client.py"],
      "evidence": {
        "httpx/_models.py": ["symbol_assertion"]
      }
    },
    "tests/client/test_event_hooks.py": {
      "primary_targets": ["httpx/_client.py"],
      "secondary_targets": [],
      "evidence": {
        "httpx/_client.py": ["symbol_assertion"]
      }
    },
    "tests/client/test_headers.py": {
      "primary_targets": ["httpx/_models.py"],
      "secondary_targets": ["httpx/_client.py"],
      "evidence": {
        "httpx/_models.py": ["symbol_assertion"]
      }
    },
    "tests/client/test_properties.py": {
      "primary_targets": ["httpx/_client.py"],
      "secondary_targets": [],
      "evidence": {
        "httpx/_client.py": ["symbol_assertion"]
      }
    },
    "tests/client/test_proxies.py": {
      "primary_targets": ["httpx/_client.py"],
      "secondary_targets": [],
      "evidence": {
        "httpx/_client.py": ["symbol_assertion"]
      }
    },
    "tests/client/test_queryparams.py": {
      "primary_targets": ["httpx/_models.py"],
      "secondary_targets": [],
      "evidence": {
        "httpx/_models.py": ["symbol_assertion"]
      }
    },
    "tests/client/test_redirects.py": {
      "primary_targets": ["httpx/_client.py"],
      "secondary_targets": [],
      "evidence": {
        "httpx/_client.py": ["symbol_assertion"]
      }
    },
    "tests/models/test_cookies.py": {
      "primary_targets": ["httpx/_models.py"],
      "secondary_targets": [],
      "evidence": {
        "httpx/_models.py": ["symbol_assertion"]
      }
    },
    "tests/models/test_headers.py": {
      "primary_targets": ["httpx/_models.py"],
      "secondary_targets": [],
      "evidence": {
        "httpx/_models.py": ["symbol_assertion"]
      }
    },
    "tests/models/test_queryparams.py": {
      "primary_targets": ["httpx/_models.py"],
      "secondary_targets": [],
      "evidence": {
        "httpx/_models.py": ["symbol_assertion"]
      }
    },
    "tests/models/test_requests.py": {
      "primary_targets": ["httpx/_models.py"],
      "secondary_targets": [],
      "evidence": {
        "httpx/_models.py": ["symbol_assertion"]
      }
    },
    "tests/models/test_responses.py": {
      "primary_targets": ["httpx/_models.py"],
      "secondary_targets": [],
      "evidence": {
        "httpx/_models.py": ["symbol_assertion"]
      }
    },
    "tests/models/test_url.py": {
      "primary_targets": ["httpx/_urls.py"],
      "secondary_targets": ["httpx/_urlparse.py"],
      "evidence": {
        "httpx/_urls.py": ["symbol_assertion"]
      }
    },
    "tests/models/test_whatwg.py": {
      "primary_targets": ["httpx/_urlparse.py"],
      "secondary_targets": [],
      "evidence": {
        "httpx/_urlparse.py": ["direct_import"]
      }
    }
  }
}
```

## Summary

| Metric | Value |
|--------|-------|
| Test files | 30 |
| Test files with primary targets | 30 |
| Unique production files targeted | 14 |
| Barrel-only imports (needs_manual_review) | 28/30 (93%) |
| Direct imports | 2/30 (7%) |
| Filename convention matches | 16/30 (53%) |
