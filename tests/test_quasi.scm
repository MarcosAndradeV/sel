(define x '(2 3))
(define y `(1 ~@x 4))
(println y)
(if (empty? (cdr (cdr (cdr (cdr y))))) (println "OK") (println "FAIL"))
