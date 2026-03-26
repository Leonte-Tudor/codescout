# Language Patterns

Per-language anti-patterns and correct patterns for this project's languages.
Each section lists the top 5 mistakes LLMs make and the top 5 idiomatic patterns.

### Kotlin

**Anti-patterns (Don't → Do):**
1. `!!` (not-null assertion) overuse → `?.let`, `?:`, `?.` chaining, or redesign to eliminate nullability
2. `GlobalScope.launch`/`async` → lifecycle-bound scopes: `viewModelScope`, `lifecycleScope`, injected `CoroutineScope`
3. `runBlocking` in production code → only for `main()` and tests, use suspend functions
4. Mutable `var` in data classes → `val` + `List` (not `MutableList`), immutability by default
5. `enum` when sealed class is needed → `sealed class`/`sealed interface` for state with per-variant data

**Correct patterns:**
1. `val` over `var`, `List` over `MutableList` — expose read-only interfaces
2. Structured concurrency: `coroutineScope { launch { a() }; launch { b() } }`
3. Sealed class/interface for all state and result types
4. `Sequence` for large collections with chained operations
5. `require`/`check`/`error` for preconditions: `require(age >= 0) { "Age must be non-negative" }`

---

### Java

**Anti-patterns (Don't → Do):**
1. `@Autowired` field injection → constructor injection with `final` fields (Spring 4.3+ auto-infers)
2. `Optional.get()` without check → `orElseThrow(() -> new NotFoundException(id))`, Optional for return types only
3. `throws Exception` / bare catches → declare and catch specific exceptions, log with context
4. `Date`/`Calendar`/`SimpleDateFormat` → `java.time`: `LocalDate`, `ZonedDateTime`, `DateTimeFormatter`
5. Raw types `List items` → `List<String> items = new ArrayList<>()`

**Correct patterns:**
1. Records for data carriers (Java 16+): `public record UserDto(String name, String email) {}`
2. Sealed classes + pattern matching (Java 17+/21+) with switch expressions
3. Text blocks `"""` for multi-line strings (Java 15+)
4. Pattern matching instanceof (Java 16+): `if (obj instanceof String s) { s.length(); }`
5. Immutable collections: `List.of()`, `Map.of()`, `Set.of()`
