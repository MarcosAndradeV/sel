;; Record Structures and Functional Programming in sel Lisp
;; This example demonstrates record syntax, callable records, functional updates, and error monads.

(println "=== Sel Records and Error Handling Demo ===")

;; Create a 2D point record
(define p1 {x 10 y 20})
(println "Created record p1:" p1)
(println "  p1 is a record? ->" (record? p1))
(println "  p1 keys:" (rkeys p1))
(println "  p1 values:" (rvals p1))

;; Retrieve values using callable syntax
(println "  x-coord via (p1 'x):" (p1 'x))
(println "  y-coord via ('y p1):" ('y p1))

;; Functional update: rset yields a new record (the original remains immutable!)
(define p2 (rset p1 'x 100))
(println "Updated record p2 (x=100):" p2)
(println "Original p1 (unmodified):" p1)

;; Check if a field exists
(println "  Does p2 contain field 'z'? ->" (rcontains? p2 'z))

;; Let's write a function that performs division safely using Monadic Error Values
(define safe-divide
  (lambda (num denom)
    (if (= denom 0)
        (err "Division by zero is forbidden!")
        (ok (/ num denom)))))

(println "\nTesting Monadic Error Values:")

;; Attempt 1: Successful division
(define res1 (safe-divide 100 4))
(if (ok? res1)
    (println "  Success! 100 / 4 = " (unwrap res1))
    (println "  Failed! Error: " (error-value res1)))

;; Attempt 2: Forbidden division (will return an err tuple instead of crashing)
(define res2 (safe-divide 100 0))
(if (ok? res2)
    (println "  Success!" (unwrap res2))
    (println "  Failed! Error message: \"" (error-value res2) "\""))
