---
name: go-best-practices
description: Go coding best practices. Use when writing or reviewing Go code. Covers error handling, concurrency, and idiomatic patterns.
license: MIT
metadata:
  author: plan-cascade
  version: "1.0.0"
---

# Go Best Practices

## Code Style

| Rule | Guideline |
|------|-----------|
| Formatter | `gofmt` or `goimports` |
| Linter | `golangci-lint` |
| Naming | Short, clear; avoid stuttering |
| Comments | Godoc for exported items |

## Error Handling

| Rule | Guideline |
|------|-----------|
| Always check | Never ignore errors |
| Wrap context | `fmt.Errorf("ctx: %w", err)` |
| Sentinel errors | `var ErrNotFound = errors.New(...)` |

```go
func Load(path string) (*Config, error) {
    data, err := os.ReadFile(path)
    if err != nil {
        return nil, fmt.Errorf("load config %s: %w", path, err)
    }
    // ...
}
```

## Project Structure

```
cmd/appname/main.go
internal/config/
internal/service/
go.mod
```

## Concurrency

| Pattern | Usage |
|---------|-------|
| `context.Context` | Cancellation, timeouts |
| `sync.WaitGroup` | Wait for goroutines |
| `errgroup.Group` | Goroutines with errors |

```go
func Process(ctx context.Context, items []Item) error {
    for _, item := range items {
        select {
        case <-ctx.Done():
            return ctx.Err()
        default:
            if err := process(item); err != nil { return err }
        }
    }
    return nil
}
```

## Anti-Patterns

| Avoid | Use Instead |
|-------|-------------|
| Naked returns | Explicit returns |
| `panic` for errors | Return errors |
| Large interfaces | Small, focused |
| `init()` | Explicit init |

## Testing (Table-Driven)

```go
func TestParse(t *testing.T) {
    tests := []struct{ name, input string; want int; wantErr bool }{
        {"valid", "42", 42, false},
        {"invalid", "abc", 0, true},
    }
    for _, tt := range tests {
        t.Run(tt.name, func(t *testing.T) {
            got, err := Parse(tt.input)
            if (err != nil) != tt.wantErr { t.Errorf("err=%v, want=%v", err, tt.wantErr) }
            if got != tt.want { t.Errorf("got=%v, want=%v", got, tt.want) }
        })
    }
}
```
