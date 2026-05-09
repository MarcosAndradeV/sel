(defmacro when (test &body)
  (list 'if test (cons 'begin body)))

(define x 10)
(when (> x 5)
  (display "x is greater than 5")
  (newline)
  (set! x 20))

(display "x is now: ")
(display x)
(newline)

(defmacro unless (test &body)
  (list 'if test 'nil (cons 'begin body)))

(define y 3)
(unless (> y 5)
  (display "y is NOT greater than 5")
  (newline)
  (set! y 10))

(display "y is now: ")
(display y)
(newline)
