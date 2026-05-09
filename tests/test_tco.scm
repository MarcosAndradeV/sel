(define sum (lambda (n acc)
  (if (= n 0)
      acc
      (sum (- n 1) (+ n acc)))))

(println (sum 100000 0))
