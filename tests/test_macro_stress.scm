;; Stress test for sel's macro system

(println "Testing static-fact...")
;; 1. Compile-time evaluation helper
(define (compile-time-fact n)
  (if (<= n 1)
      1
      (* n (compile-time-fact (- n 1)))))

(defmacro static-fact (n)
  (compile-time-fact n))

(assert (= (static-fact 5) 120))
(assert (= (static-fact 10) 3628800))

(println "Testing when-let...")
;; 2. let-binding macros with quasiquote construction
(defmacro when-let (binding &body)
  (define var (car binding))
  (define val (car (cdr binding)))
  `(let ((~var ~val))
     (when ~var
       ~@body)))

(define test-val nil)
(when-let (x 42)
  (set! test-val x))
(println "First when-let done. test-val =" test-val)
(assert (= test-val 42))
(println "First assert passed")

(when-let (y #f)
  (set! test-val 999))
(println "Second when-let done. test-val =" test-val)
(assert (= test-val 42)) ; Should remain 42 because #f is falsy
(println "Second assert passed")

(println "Testing nested-add...")
;; 3. Deep recursion macro to stress-test nested macro expansions
;; This macro expands recursively N times to build a nested structure of operations
(defmacro nested-add (n expr)
  (if (<= n 0)
      expr
      `(nested-add ~(- n 1) (+ 1 ~expr))))

(assert (= (nested-add 10 0) 10))

(println "Testing with-captured-var...")
;; 4. Macro hygiene validation (checking if unhygienic bindings behave predictably)
(defmacro with-captured-var (body-expr)
  `(let ((captured-val 100))
     ~body-expr))

;; Verify we can read the captured variable (unhygienic behavior)
(assert (= (with-captured-var captured-val) 100))

(println "Macro stress test completed successfully!")
