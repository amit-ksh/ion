# Dynamic CSS names in portable SVG documents

Researched 2026-10-16 against primary specifications and their linked tests. This source research guided the [computed-name serializer increment and its executed checks](92-portable-computed-css-names.md). The research alone is not evidence of phone browser acceptance. It supplements chapters 85 and 88.

## Recommended implementation boundary

Keep native substitution and validation. Extend literal name slots and proven literal fallbacks; preserve `ident()` when rewriting its result so an unsupported engine still follows its original fallback behavior. Do not globally rename custom-property token data or install consumer-only alias properties. General arbitrary computed names remain open until their isolation is actually implemented.

## Variables: timing, inheritance and cycles

Unregistered custom properties contain arbitrary token streams. Their own `var()` references resolve before inheritance. Missing/invalid values select a supplied fallback; an empty valid value differs from the guaranteed-invalid value. Stable Variables 1 defines cycle edges even for fallback references. CSS-wide keywords on a custom declaration perform defaulting rather than becoming ordinary stored data. These facts prevent a serializer from treating custom properties as macro source text. [CSS Variables 1, sections 2–3](https://www.w3.org/TR/css-variables-1/#defining-variables).

**Derived counterexample:** in this authored example the child inherits the already resolved value `pulse` for `--chosen`. An alias first introduced on the child must not re-evaluate the producer against `--base: spin`.

```css
.parent {
  --base: pulse;
  --chosen: var(--base);
}
.child {
  --base: spin;
  animation-name: var(--chosen);
}
```

A mirrored graph would need declarations at their original elements, cascade positions, priorities, conditions and keyframes; every dependency and registration would need the same semantics. That still does not solve contextual grammar below.

The current Variables 2 Editor’s Draft (9 October 2026) gives `var()` a broader argument grammar. Its first argument is substituted and then parsed as a custom-property name; examples include nested `var(var(--name))`. Cycle handling delegates to substitution contexts. Treat this as a separate evolving contract from Variables 1, not a reason to flatten new functions into legacy syntax. A literal `var(ident("--tone"), red)` can be renamed inside its existing `ident()` wrapper. Unknown computed property names need further work. [CSS Variables 2, section 3](https://drafts.csswg.org/css-variables-2/#using-variables).

## Why role-specific aliases are not general

Font families may comprise multiple unquoted identifiers. The complete sequence becomes one family name with spaces; a quoted name mixed with unquoted tokens is invalid. Generic keywords have different meaning from quoted literal families. [CSS Fonts 4, family-name syntax](https://www.w3.org/TR/css-fonts-4/#family-name-syntax).

**Derived counterexamples:**

```css
--a: Report;
--b: Display;
font-family:
  var(--a) var(--b),
  serif; /* family Report Display */
font-family:
  var(--missing, Report) Display,
  serif;
```

Mapping either producer or fallback independently to a complete private family breaks the combined name. Dynamic font suffixes can also select an installed family. Always adding a private prefix loses that installed family; adding an extra original-family fallback can change per-character glyph selection. Literal collected-family membership remains the sound immediate boundary.

Animation shorthand gives keywords to still-unfilled non-name slots before treating them as names. Thus `linear` can be a timing function or a keyframe name depending on surrounding tokens. [CSS Animations 1, section 3.10](https://www.w3.org/TR/css-animations-1/#animation-shorthand).

**Derived counterexamples:**

```css
--x: linear;
animation: 1s var(--x); /* timing, no named animation */
animation: 1s linear var(--x); /* second linear can be the name */
```

A fallback transformer must know the complete branch and parser state. Partial custom-property values can supply commas, multiple components, or no components. A renamed alias valid for one consumer is not necessarily valid for another.

## Registration constraints

`@property` requires `syntax` and `inherits`. `initial-value` is optional only for `syntax: "*"`; otherwise it must match the syntax and be computationally independent. A `var()`-dependent initial value is not independent. Registered properties parse liberally initially, then enforce their registered type at computed-value time and substitute the resulting computed value. Preserve registration order, defaults, inheritance and type; do not silently replace a typed property with an unregistered alias. Registration maps are document-global even across shadow trees. [Properties and Values API 1, sections 2–4](https://www.w3.org/TR/css-properties-values-api-1/).

## Literal and computed ident() contract

`ident()` concatenates resolved identifier, string and integer pieces into one identifier. Its argument grammar is looser at parse time; semantic failure can happen later. The final result is parsed in its property context, including CSS-wide/default/other keyword rules. Arbitrary substitution functions apply within property values, not rule names, declaration names or descriptors unless explicitly allowed. [CSS Values 5, ident and substitution algorithms](https://drafts.csswg.org/css-values-5/#ident).

Implement a bounded literal evaluator only for decoded strings/identifiers and provable integer tokens. Preserve unresolved functions, `var()`, `attr()`, numeric math and unknown syntax. Rewrite as an `ident()` expression, rather than replacing the function with a bare token or quoted family. Empty or invalid expressions must not become valid merely because a private prefix was added.

A **derived safe affix case** is a single-name slot whose first decoded literal segment is guaranteed nonempty and cannot start any reserved keyword. Adding the private namespace inside that existing segment preserves unknown-argument evaluation and failure timing. Dashed names require fixed `--` plus at least one guaranteed character. This does not establish safety for shorthand, partial families, arbitrary variable streams, or unknown fonts.

The linked tests distinguish parse-invalid empty/comma forms from parse-valid expressions that fail at computed-value time. They also cover integer math and escaping a generated identifier beginning with a digit. [WPT parsing](https://github.com/web-platform-tests/wpt/blob/master/css/css-values/ident-function-parsing.html), [WPT computed values](https://github.com/web-platform-tests/wpt/blob/master/css/css-values/ident-function-computed.html).

Counter-style names require additional care: predefined names canonicalize ASCII case, while author names are case-sensitive. A dynamic affix such as `ident("lower-" var(--suffix))` can select a native predefined style, so unconditional private prefixing loses that baseline. [CSS Counter Styles 3, section 3](https://www.w3.org/TR/css-counter-styles-3/#the-counter-style-rule).

## Scope is not a substitute for name isolation

`@scope` explicitly leaves nested global name-defining rules such as keyframes, font faces and layers unscoped. Scoping selectors cannot isolate those definitions or their dynamic references. [CSS Cascade 6, scoped style rules](https://www.w3.org/TR/css-cascade-6/#scoped-style-rules).

Shadow-tree names and references carry actual tree identity, including through inheritance; that differs from a selector prefix or `@scope`. Moving imported SVG definitions into real shadow roots would need a separate native SVG `use`/resource and browser contract. It cannot be inferred from transformed selectors. [CSS Shadow 1, name definitions and inheritance](https://drafts.csswg.org/css-shadow-1/#name-scoping).

## Verification needed for the increment

- Name definitions and uses agree across two imported documents, inline styles, pinned imports and presentation values.
- Whole-slot literal fallbacks and literal `ident()` follow the private identity; opaque custom data and primary arbitrary variable streams remain unchanged.
- Generic/none/CSS-wide keywords, invalid expressions, integer syntax, escaped strings, nested unknown functions and unsupported-function fallbacks retain their original behavior.
- Counter builtins and installed-font fallback remain native.
- Partial-family and shorthand-ambiguity counterexamples remain regression cases; they are not represented as completed arbitrary-name support.
- Independent parser checks establish serialization grammar only. Actual computed behavior, glyph selection and physical phone acceptance require the corresponding engine/device checks.

Engine support must be checked per property and browser version. The WebKit tracker records an August 2026 change from earlier property-specific handling to arbitrary substitution; a tracker landing is not proof that the owner’s iPhone or Android WebView supports every requested use. [WebKit implementation tracker](https://bugs.webkit.org/show_bug.cgi?id=284895).
