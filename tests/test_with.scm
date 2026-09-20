;; tests/test_with.scm - Tests for Railway-Oriented `with` construct in sel

;; 1. Happy path: chained successful matches
(define (fetch-user id)
  (if (= id 1)
      (list 'ok {name "Alice" role 'admin})
      (list 'error 'user-not-found)))

(define (fetch-permissions role)
  (if (eq? role 'admin)
      (list 'ok '(read write delete))
      (list 'error 'invalid-role)))

(define res-happy
  (with (((list 'ok {name n role r}) (fetch-user 1))
         ((list 'ok perms) (fetch-permissions r)))
    :do
    (list n perms)))

(assert (eq? (car res-happy) "Alice"))
(assert (eq? (car (cdr res-happy)) '(read write delete)))

;; 2. Early-exit without :else (returns the mismatched value directly)
(define res-early-no-else
  (with (((list 'ok user) (fetch-user 999))
         ((list 'ok perms) (fetch-permissions 'admin)))
    :do
    "Should not reach here"))

(assert (eq? res-early-no-else (list 'error 'user-not-found)))

;; 3. Early-exit with :else clause matching
(define (run-with-else id)
  (with (((list 'ok {role r}) (fetch-user id))
         ((list 'ok perms) (fetch-permissions r)))
    :do
    (list 'success perms)
    :else
    ((list 'error 'user-not-found) :do "User missing from database")
    ((list 'error 'invalid-role) :do "Role not recognized")
    (_ :do "Unknown error")))

(assert (eq? (run-with-else 1) (list 'success '(read write delete))))
(assert (eq? (run-with-else 999) "User missing from database"))

;; 4. Clauses with guard conditions (:when / :where)
(define (safe-divide a b)
  (with ((b-val :when (!= b-val 0) b)
         (res (/ a b-val)))
    :do
    res
    :else
    (0 :do 'division-by-zero)))

(assert (= (safe-divide 10 2) 5))
(assert (eq? (safe-divide 10 0) 'division-by-zero))

;; 5. Flat clause syntax (without wrapping clauses list)
(define res-flat
  (with ((list 'ok u) (fetch-user 1))
        ((list 'ok p) (fetch-permissions 'admin))
    :do
    'all-ok
    :else
    (_ :do 'failed)))

(assert (eq? res-flat 'all-ok))
