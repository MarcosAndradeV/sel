;; examples/modern_functional.scm
;; A showcase of Elixir/OCaml-inspired modern functional programming in Sel
;; Features demonstrated:
;; 1. Multi-clause `defn` with pattern matching and guards
;; 2. Pipeline operator `|>` for collections and transformations
;; 3. Thread-first `->` for record manipulations
;; 4. Railway-oriented `with` with :do and :else
;; 6. List comprehension `for` with :let and :when

(println "=== Sel Modern Functional Showcase ===")
(newline)

;; ----------------------------------------------------
;; 1. Multi-Clause Function Definitions (defn)
;; ----------------------------------------------------
(println "--- 1. Multi-clause defn with pattern matching and guards ---")

;; Calculate discount rate based on user tier and order amount
(defn (discount-rate tier amount)
  (('vip amount) :where (>= amount 1000) :do 0.25)
  (('vip amount) :do 0.15)
  (('member amount) :where (>= amount 500) :do 0.10)
  (('member amount) :do 0.05)
  ((_ _) :do 0.0))

(println "VIP discount on $1200:" (discount-rate 'vip 1200))
(println "Member discount on $600:" (discount-rate 'member 600))
(println "Guest discount on $100:" (discount-rate 'guest 100))
(newline)

;; ----------------------------------------------------
;; 2. Pipelines: Thread-last (|>) and Thread-first (->)
;; ----------------------------------------------------
(println "--- 2. Pipelines (|>) and (->) ---")

(define inventory
  (list {id 1 name "Mechanical Keyboard" price 120 stock 15}
        {id 2 name "Ergonomic Mouse" price 60 stock 0}
        {id 3 name "4K Monitor" price 450 stock 8}
        {id 4 name "USB-C Hub" price 35 stock 50}))

;; Thread-last pipeline (|>) for processing collections:
(define in-stock-summary
  (|> inventory
      (filter \(item) (> (rget item 'stock) 0))
      (map \(item) (list (rget item 'name)
                         'total (* (rget item 'price) (rget item 'stock))))))

(println "In-stock inventory values:")
(for (entry in-stock-summary) :do (println "  -" entry))
(newline)

;; Thread-first pipeline (->) for modifying a record:
(define user {id 101 name "Alice" tier 'vip balance 1500})
(define updated-user
  (-> user
      (assoc 'balance 1300)
      (assoc 'last-login "2026-09-20")))

(println "Updated user balance:" (rget updated-user 'balance))
(println "Updated user last login:" (rget updated-user 'last-login))
(newline)

;; ----------------------------------------------------
;; 3. Railway-Oriented Programming with `with`
;; ----------------------------------------------------
(println "--- 3. Railway-oriented `with` construct ---")

(define (find-product pid)
  (if (= pid 1)
      (list 'ok {id 1 name "Mechanical Keyboard" price 120 stock 10})
      (if (= pid 2)
          (list 'ok {id 2 name "Ergonomic Mouse" price 60 stock 0})
          (list 'error 'product-not-found))))

(define (check-stock prod quantity)
  (if (>= (rget prod 'stock) quantity)
      (list 'ok prod)
      (list 'error 'out-of-stock)))

(define (charge-account user-rec cost)
  (if (>= (rget user-rec 'balance) cost)
      (list 'ok (assoc user-rec 'balance (- (rget user-rec 'balance) cost)))
      (list 'error 'insufficient-funds)))

;; Seamless railway validation: short-circuits on first failure
(define (process-order user-rec pid quantity)
  (with (((list 'ok prod) (find-product pid))
         ((list 'ok in-stock) (check-stock prod quantity))
         ((list 'ok updated-u) (charge-account user-rec (* (rget in-stock 'price) quantity))))
    :do
    (list 'order-placed {product (rget prod 'name)
                         quantity quantity
                         new-balance (rget updated-u 'balance)})
    :else
    ((list 'error 'product-not-found) :do "Error: Product does not exist")
    ((list 'error 'out-of-stock) :do "Error: Item is currently out of stock")
    ((list 'error 'insufficient-funds) :do "Error: Account balance is too low")
    (other :do "Error: Unexpected issue")))

(println "Success order:" (process-order user 1 2))
(println "Out of stock order:" (process-order user 2 1))
(println "Unknown product order:" (process-order user 999 1))
(newline)

;; ----------------------------------------------------
;; 4. List Comprehensions (`for`)
;; ----------------------------------------------------
(println "--- 4. List Comprehensions (`for`) ---")

;; Filter and calculate squares of even numbers:
(define evens-squared
  (for (n (range 10))
    :when (= (mod n 2) 0)
    :do (* n n)))

(println "Evens squared:" evens-squared)

;; Cartesian product with local :let bindings and condition:
(define grid-coords
  (for (x '(1 2 3))
       (y '(10 20 30))
    :let ((sum (+ x y)))
    :when (> sum 20)
    :do {x x y y sum sum}))

(println "Grid coordinates where x + y > 20:")
(for (coord grid-coords)
  :do (println " " coord))

(newline)
(println "=== All modern functional features executed successfully! ===")
