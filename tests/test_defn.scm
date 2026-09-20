;; tests/test_defn.scm - Tests for multi-clause defn in sel

;; 1. Single-argument defn with shorthand name
(defn fib
  (0 :do 0)
  (1 :do 1)
  (n (+ (fib (- n 1)) (fib (- n 2)))))

(assert (= (fib 0) 0))
(assert (= (fib 1) 1))
(assert (= (fib 2) 1))
(assert (= (fib 3) 2))
(assert (= (fib 4) 3))
(assert (= (fib 5) 5))
(assert (= (fib 6) 8))

;; 2. Explicit single-argument signature with :where and :when guards
(defn (classify n)
  (0 :do "zero")
  (n :where (< n 0) :do "negative")
  (n :when (> n 0) :do "positive"))

(assert (eq? (classify 0) "zero"))
(assert (eq? (classify -42) "negative"))
(assert (eq? (classify 99) "positive"))

;; 3. Multi-argument defn
(defn (gcd a b)
  ((a 0) :do a)
  ((a b) :do (gcd b (mod a b))))

(assert (= (gcd 48 18) 6))
(assert (= (gcd 54 24) 6))
(assert (= (gcd 7 3) 1))

;; 4. Record destructuring with guards in defn
(defn (handle-msg msg)
  ({type 'login user u} :where (eq? u "admin")
   :do "admin-session")
  ({type 'login user u}
   :do "user-session")
  ({type 'logout}
   :do "logged-out")
  (_ :do "unknown"))

(assert (eq? (handle-msg {type 'login user "admin"}) "admin-session"))
(assert (eq? (handle-msg {type 'login user "alice"}) "user-session"))
(assert (eq? (handle-msg {type 'logout}) "logged-out"))
(assert (eq? (handle-msg 123) "unknown"))

;; 5. Tail Call Optimization (TCO) with defn
(defn (countdown-defn n acc)
  ((0 acc) :do acc)
  ((n acc) :do (countdown-defn (- n 1) (+ acc 1))))

(assert (= (countdown-defn 20000 0) 20000))
