;; Sel Scheme Records Feature Tests

(define r {a 1 b 2})

;; 1. Basic rget tests
(assert (= (rget r 'a) 1))
(assert (= (rget r 'b) 2))
(assert (nil? (rget r 'c)))

;; 2. rset tests (functional updates & dynamic insertion)
(define r2 (rset r 'a 10))
(assert (= (rget r2 'a) 10))
(assert (= (rget r2 'b) 2))
(assert (= (rget r 'a) 1)) ;; Verify immutability of r

(define r3 (rset r 'c 3))
(assert (= (rget r3 'c) 3))
(assert (= (rget r3 'a) 1))
(assert (= (rget r3 'b) 2))
(assert (nil? (rget r 'c))) ;; Verify immutability of r

;; 3. Record callable syntax
(assert (= (r 'a) 1))
(assert (= (r 'b) 2))
(assert (nil? (r 'c)))

(define r4 (r 'a 20))
(assert (= (r4 'a) 20))
(assert (= (r4 'b) 2))

;; 4. Symbol callable syntax
(assert (= ('a r) 1))
(assert (= ('b r) 2))
(assert (nil? ('c r)))

(define r5 ('a r 30))
(assert (= ('a r5) 30))
(assert (= ('b r5) 2))

;; 5. Native helpers (rdel, rcontains?, rkeys, rvals)
(define r6 (rdel r3 'b))
(assert (nil? (rget r6 'b)))
(assert (= (rget r6 'a) 1))
(assert (= (rget r6 'c) 3))

(assert (rcontains? r 'a))
(assert (not (rcontains? r 'c)))

(assert (eq? (rkeys r) '(a b)))
(assert (eq? (rvals r) '(1 2)))

;; 6. Standard library assoc/dissoc wrappers
(define r7 (assoc r 'c 3))
(assert (= (rget r7 'c) 3))

(define r8 (dissoc r 'b))
(assert (nil? (rget r8 'b)))

;; 7. record? predicate
(assert (record? r))
(assert (not (record? 'a)))
(assert (not (record? 123)))
(assert (not (record? '())))

;; 8. Macro expansion and quasiquote dynamic compilation inside records
(defmacro make-point (x y)
  `{x ~x y ~y})

(define p (make-point 10 20))
(assert (= (p 'x) 10))
(assert (= (p 'y) 20))
(assert (record? p))

(println "All record tests passed successfully!")
