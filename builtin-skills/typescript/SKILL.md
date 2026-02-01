---
name: typescript-best-practices
description: TypeScript/Node.js best practices. Use when writing or reviewing TypeScript code. Covers type safety, async patterns, and error handling.
license: MIT
metadata:
  author: plan-cascade
  version: "1.0.0"
---

# TypeScript Best Practices

## Code Style

| Rule | Guideline |
|------|-----------|
| Formatter | Prettier |
| Linter | ESLint + `@typescript-eslint` |
| Strict mode | `strict: true` in tsconfig |

## Type Safety

| Rule | Guideline |
|------|-----------|
| Avoid `any` | Use `unknown` + narrowing |
| Type guards | Custom predicates |
| Discriminated unions | For variants |

```typescript
type Result<T> = { success: true; data: T } | { success: false; error: Error };

function isUser(v: unknown): v is User {
  return typeof v === 'object' && v !== null && 'id' in v;
}
```

## Error Handling

```typescript
class ApiError extends Error {
  constructor(message: string, public status: number) {
    super(message);
    this.name = 'ApiError';
  }
}

async function fetchUser(id: string): Promise<User> {
  const res = await fetch(`/api/users/${id}`);
  if (!res.ok) throw new ApiError('Failed', res.status);
  return res.json();
}
```

## Project Structure

```
src/{index.ts, types/, services/, utils/}
tests/
package.json, tsconfig.json
```

## Async Patterns

| Pattern | Usage |
|---------|-------|
| `Promise.all` | Parallel operations |
| `AbortController` | Cancellation |

```typescript
const [user, settings] = await Promise.all([fetchUser(id), fetchSettings(id)]);
```

## Anti-Patterns

| Avoid | Use Instead |
|-------|-------------|
| `as` assertions | Type guards |
| `!` non-null | `?.` and `??` |
| `any` | `unknown` + narrowing |

```typescript
// Bad: data as User, user!.name
// Good:
if (isUser(data)) { /* data is User */ }
const name = user?.name ?? 'Unknown';
```

## Testing (Vitest)

```typescript
describe('Service', () => {
  it('should create', async () => {
    const mock = { save: vi.fn().mockResolvedValue({ id: '1' }) };
    const result = await new Service(mock).create({ name: 'Test' });
    expect(result.id).toBe('1');
  });
});
```
