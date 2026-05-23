(defun sum (n acc)
    (if (= n 0)
        acc
        (sum (- n 1) (+ n acc))))

(assert (eq? (sum 100000 0) 5000050000))
