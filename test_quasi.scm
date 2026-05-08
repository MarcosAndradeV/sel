(define x '(2 3))
(define y `(1 ~@x 4))
(print y)
(if (empty? (cdr (cdr (cdr (cdr y))))) (print "OK") (print "FAIL"))
