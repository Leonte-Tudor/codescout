# Language Patterns

Per-language anti-patterns and correct patterns for this project's languages.
Each section lists the top 5 mistakes LLMs make and the top 5 idiomatic patterns.

### TypeScript

**Anti-patterns (Don't → Do):**
1. `any` type overuse → `unknown` when type is uncertain, Zod schemas for external data
2. Type assertion `as` abuse / `as unknown as T` → type guards, proper narrowing
3. Missing discriminated unions → model domain states with `'kind'`/`'type'` discriminant, `satisfies never` for exhaustiveness
4. Non-null assertion `!` abuse → handle null/undefined with narrowing, optional chaining, type guards
5. Enums → `as const` objects or string literal union types

**Correct patterns:**
1. Strict tsconfig: `strict: true`, `noUncheckedIndexedAccess`, `exactOptionalPropertyTypes`
2. Explicit return types on exported functions
3. Zod schema validation for external data — derive types with `z.infer<typeof Schema>`
4. Discriminated unions with exhaustiveness: `default: throw new Error(\`Unhandled: ${x satisfies never}\`)`
5. `interface` for object shapes, `type` for unions/intersections/mapped types

---

### JavaScript

**Anti-patterns (Don't → Do):**
1. Missing Promise error handling → every `.then()` needs `.catch()`, every `async/await` needs try/catch
2. Stale closures in React hooks → ensure exhaustive dependency arrays in useEffect/useCallback/useMemo
3. Event listener / timer memory leaks → cleanup with `removeEventListener`, `clearInterval`, `AbortController`
4. `var` declarations → `const` by default, `let` only for reassignment
5. Loose equality `==` → always `===` and `!==`

**Correct patterns:**
1. Proper useEffect async: define async inside effect, call it, return cleanup with AbortController
2. `const` by default, destructuring at function boundaries
3. Named exports over default exports — aids tree-shaking and refactoring
4. Template literals over string concatenation
5. `jsconfig.json` with `checkJs: true` for type safety in JS projects
