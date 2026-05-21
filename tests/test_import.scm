(import point)
(assert (eq? (point/new-point 1 2) {x 1 y 2}))

;; Test inline :as alias
(import point :as p)
(assert (eq? (p/new-point 3 4) {x 3 y 4}))

;; Test nested list with :as
(import (point :as pt))
(assert (eq? (pt/new-point 5 6) {x 5 y 6}))

;; Test nested list shorthand
(import (point pnt))
(assert (eq? (pnt/new-point 7 8) {x 7 y 8}))
