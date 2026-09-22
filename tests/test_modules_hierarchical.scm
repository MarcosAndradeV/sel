;; Test Hierarchical and String Module Imports

;; 1. Subdirectory import by symbol
(import modules_test/math/geom)
(assert (= (geom/square 4) 16))
(assert (= (geom/hypot 3 4) 5.0))

;; 2. Directory package (mod.scm) import
(import modules_test/pkg)
(assert (eq? (pkg/greet "Alice") "Hello from pkg, Alice!"))

;; 3. String path import with alias
(import "modules_test/math/geom" :as g)
(assert (= (g/square 5) 25))

;; 4. Nested string import with alias
(import ("modules_test/pkg" :as p))
(assert (eq? (p/greet "Bob") "Hello from pkg, Bob!"))

;; 5. SEL_PATH resolution
(set-env! "SEL_PATH" "tests/modules_test")
(import math/geom :as mg)
(assert (= (mg/hypot 6 8) 10.0))

(import pkg :as mp)
(assert (eq? (mp/greet "World") "Hello from pkg, World!"))
