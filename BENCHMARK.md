# Source
## 1 > 2

- Merged two different `map`s for splitting into one, removing an additional allocation.
- Tried `filter_map`, but found the performance to be **~5%** worse than separate `map` and `filter`.

``` diff
     pub fn from_lines(lines: Lines) -> Self {
-        // remove everything from '(' to end
-        let uncommented = lines.map(|line| {
-            line.split('(')
-                .next()
-                .expect("At least one element must exist after splitting.")
-                .to_string()
-        });
-
-        // remove everything from ';' to end and trim
-        let nocolon = uncommented.map(|line| {
-            line.split(';')
-                .next()
-                .expect("At least one element must exist after splitting.")
-                .trim()
-                .to_string()
-        });
-
-        // remove deleted blocks and control character
-        let filtered = nocolon
-            .filter(|line| !line.is_empty() && !line.starts_with('/') && !line.starts_with('%'));
+        // filter_map performs worse here
+        let sanitized = lines
+            .map(|line| {
+                line.split(['(', ';'])
+                    .next()
+                    .expect("At least one element must exist after splitting.")
+                    .trim() // remove everything from '(' or ';' to end
+            })
+            .filter(|line| !line.is_empty() && !line.starts_with('/') && !line.starts_with('%')); // delete blocks and control character

         Self {
-            lines: filtered.map(Line).collect(),
+            lines: sanitized.map(|l| Line(l.to_string())).collect(),
             index: 0,
         }
     }
```

```
source                  time:   [613.61 µs 616.17 µs 618.90 µs]
                        change: [−22.821% −21.263% −20.129%] (p = 0.00 < 0.05)
                        Performance has improved.
Found 9 outliers among 100 measurements (9.00%)
  4 (4.00%) high mild
  5 (5.00%) high severe
```

# Lexer
## 1 > 2

- Removed `String` allocation by using a **slice**.

``` diff
diff --git a/src/lexer.rs b/src/lexer.rs
     fn next(&mut self) -> Option<Self::Item> {
-        self.0.next().map(|line| Block::tokenize(line.as_str()))
+        self.0.next().map(|line| Block::tokenize(line))
     }

diff --git a/src/source.rs b/src/source.rs
-    fn next(&mut self) -> Option<Self::Item> {
+    pub fn next(&mut self) -> Option<&str> {
         let line = self.lines.get(self.index)?;
         self.index += 1;

-        Some(line.clone())
+        Some(line)
     }
```

```
lexer                   time:   [990.28 µs 996.80 µs 1.0044 ms]
                        change: [−10.261% −8.9887% −7.4577%] (p = 0.00 < 0.05)
                        Performance has improved.
Found 6 outliers among 100 measurements (6.00%)
  1 (1.00%) high mild
  5 (5.00%) high severe
```

# Parser
## 1 > 2

- Removed `block.clone()` call and instead save the G and M codes in a `Vec` and parse them later.


``` diff
         let mut gcodes = GCodes::new();
         let mut mcode = None;
         let mut codes = Codes::new();
+        let mut gcodes_unparsed = Vec::new();
+        let mut mcode_unparsed = None;

-        // parse and store only non G and non M tokens
-        for token in block
-            .clone()
-            .filter(|token| token.prefix != b'G' && token.prefix != b'M')
-        {
+        // do not parse G or M codes until every other code is parsed
+        for token in block {
             let code = Code::parse(&token)?;
-            codes.push(code)?;
+
+            match token.prefix {
+                b'G' => gcodes_unparsed.push(code),
+                b'M' => {
+                    if mcode_unparsed.is_some() {
+                        return Err(ParserError::DuplicatePrefix(b'M'));
+                    }
+                    mcode_unparsed = Some(code);
+                }
+                _ => codes.push(code)?,
+            }
         }

         // parse any mcode & gcode(s)
-        for token in block.filter(|token| token.prefix == b'G' || token.prefix == b'M') {
-            let code = Code::parse(&token)?;
+        for code in gcodes_unparsed {
+            gcodes.push(GCode::parse(code, &mut codes)?)?;
+        }

-            if token.prefix == b'M' {
-                if mcode.is_some() {
-                    return Err(ParserError::DuplicatePrefix(b'M'));
-                }
-                mcode = Some(MCode::parse(code, &mut codes)?);
-            } else {
-                gcodes.push(GCode::parse(code, &mut codes)?)?;
-            }
+        if let Some(code) = mcode_unparsed {
+            mcode = Some(MCode::parse(code, &mut codes)?);
         }
```

```
parser                  time:   [1.3781 ms 1.3907 ms 1.4042 ms]
                        change: [−11.024% −9.4890% −7.7924%] (p = 0.00 < 0.05)
                        Performance has improved.
Found 5 outliers among 100 measurements (5.00%)
  4 (4.00%) high mild
  1 (1.00%) high severe
```

# Geometry
## 1 > 2

- Removed equality check for every loop.

``` diff
diff --git a/src/geometry.rs b/src/geometry.rs
index 7956882..a0b411b 100644
--- a/src/geometry.rs
+++ b/src/geometry.rs
@@ -1003,16 +1003,15 @@ impl StockInstance {
         let mut current_x = start;
         let mut current_y = start;
         let mut current_z = start;
+        let len_x = (size.x / edge).ceil() as usize;
+        let len_y = (size.y / edge).ceil() as usize;
+        let len_z = (size.z / edge).ceil() as usize;

-        let mut ret = Vec::with_capacity(
-            (size.x / edge).ceil() as usize
-                + (size.y / edge).ceil() as usize
-                + (size.z / edge).ceil() as usize,
-        );
+        let mut ret = Vec::with_capacity(len_x * len_y * len_z);

-        while current_x < size.x {
-            while current_y < size.y {
-                while current_z < size.z {
+        for _ in 0..len_x {
+            for _ in 0..len_y {
+                for _ in 0..len_z {
                     ret.push(Self {
                         center: [current_x, current_y, current_z],
                     });
```

```
geometry                time:   [3.4586 ms 3.4779 ms 3.4972 ms]
                        change: [−10.278% −9.6613% −9.0756%] (p = 0.00 < 0.05)
                        Performance has improved.
Found 18 outliers among 100 measurements (18.00%)
  11 (11.00%) low mild
  6 (6.00%) high mild
  1 (1.00%) high severe
```
