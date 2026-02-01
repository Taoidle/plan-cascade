---
name: python-best-practices
description: Python coding best practices. Use when writing or reviewing Python code. Covers type hints, error handling, and common patterns.
license: MIT
metadata:
  author: plan-cascade
  version: "1.0.0"
---

# Python Best Practices

## Code Style

| Rule | Guideline |
|------|-----------|
| Formatter | `ruff format` or `black` |
| Linter | `ruff check` with strict rules |
| Type hints | Always for public APIs |
| Docstrings | Google or NumPy style |

## Type Hints

```python
def process(items: list[str], max_count: int = 10) -> dict[str, int]: ...
def find_user(user_id: int) -> User | None: ...
```

## Error Handling

| Rule | Guideline |
|------|-----------|
| Specific exceptions | Never bare `except:` |
| Context managers | Use `with` for resources |
| Early return | Validate inputs, fail fast |

```python
# Good
try:
    result = process(data)
except ValidationError as e:
    logger.warning(f"Validation failed: {e}")
    return default
```

## Project Structure

```
src/package_name/
├── __init__.py
├── core/
└── py.typed
tests/
├── conftest.py
└── test_*.py
pyproject.toml
```

## Common Patterns

| Pattern | Usage |
|---------|-------|
| `@dataclass` | Data containers |
| `@property` | Computed attributes |
| Generators | Memory-efficient iteration |
| `functools.cache` | Memoization |

## Anti-Patterns

| Avoid | Use Instead |
|-------|-------------|
| Mutable default args | `None` default, create inside |
| `import *` | Explicit imports |
| Global state | Dependency injection |

```python
# Bad: def add(item, items=[]): ...
# Good:
def add(item, items=None):
    items = items or []
    items.append(item)
    return items
```

## Tools

| Tool | Purpose |
|------|---------|
| `pytest` | Testing |
| `ruff` | Linting + formatting |
| `mypy` | Type checking |
| `uv` | Dependencies |
