---
name: java-best-practices
description: Java coding best practices. Use when writing or reviewing Java code (17+). Covers modern features, error handling, and patterns.
license: MIT
metadata:
  author: plan-cascade
  version: "1.0.0"
---

# Java Best Practices (17+)

## Code Style

| Rule | Guideline |
|------|-----------|
| Formatter | `google-java-format` |
| Analysis | SpotBugs, Error Prone |
| Naming | Clear, no abbreviations |

## Modern Features

| Feature | Usage |
|---------|-------|
| Records | Immutable data carriers |
| Sealed classes | Restricted hierarchies |
| Pattern matching | `instanceof` with binding |
| `var` | Local type inference |

```java
public record User(String id, String name) {}

if (obj instanceof String s && !s.isEmpty()) { process(s); }

public sealed interface Result<T> permits Success, Failure {}
record Success<T>(T value) implements Result<T> {}
record Failure<T>(Exception error) implements Result<T> {}
```

## Error Handling

| Rule | Guideline |
|------|-----------|
| Specific exceptions | Not bare `Exception` |
| Try-with-resources | For `AutoCloseable` |

```java
try (var reader = new BufferedReader(new FileReader(path))) {
    return reader.lines().toList();
} catch (IOException e) {
    throw new ConfigException("Failed: " + path, e);
}
```

## Project Structure

```
src/main/java/com/example/{domain,service}/
src/test/java/
pom.xml or build.gradle
```

## Common Patterns

| Pattern | Usage |
|---------|-------|
| Optional | Null-safe returns |
| Stream API | Functional collections |
| Builder | Complex construction |

```java
public Optional<User> findById(String id) {
    return Optional.ofNullable(users.get(id));
}
```

## Anti-Patterns

| Avoid | Use Instead |
|-------|-------------|
| Returning null | `Optional<T>` |
| Mutable data | Records |
| `instanceof` chains | Pattern matching |

## Testing (JUnit 5 + AssertJ)

```java
@Test void shouldProcess() {
    var service = new Service(mockRepo);
    var result = service.process(input);
    assertThat(result.status()).isEqualTo(SUCCESS);
}
```
