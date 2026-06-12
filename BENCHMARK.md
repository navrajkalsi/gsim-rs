# Source
## 1 > 2

. Merged two different `map`s for splitting into one, removing an additional allocation.
. Tried `filter_map`, but found the performance to be **~5%** worse than separate `map` and `filter`.

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
