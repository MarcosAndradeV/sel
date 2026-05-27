(define x 10)
(when (> x 5)
  (println "x is greater than 5")
  (set! x 20))

(assert (eq? x 20))

(defmacro unless-local (test &body)
  (list 'if test 'nil (cons 'begin body)))

(define y 3)
(unless-local (> y 5)
  (println "y is NOT greater than 5")
  (set! y 10))

(assert (eq? y 10))
