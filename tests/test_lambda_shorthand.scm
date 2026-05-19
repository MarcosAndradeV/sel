(define res (-> (range 10) (map \(x) (+ x 1)) (foldr + 0)))
(assert (= res 55))
