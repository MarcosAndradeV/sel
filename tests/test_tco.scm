(defun sum (n acc)
    (if (= n 0)
        (sum (- n 1) (+ n acc))))

(println (sum 100000 0))
