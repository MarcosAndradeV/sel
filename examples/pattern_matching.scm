;; Pattern Matching in sel Lisp
;; This example demonstrates structural pattern matching with literals, lists,
;; cons, rest patterns, records, guard clauses, and match-lambda.

(println "=== Sel Pattern Matching Demo ===")

;; 1. Evaluating Expression Trees with `match`
(define (eval-expr expr)
  (match expr
    ((list '+ a b) (+ (eval-expr a) (eval-expr b)))
    ((list '* a b) (* (eval-expr a) (eval-expr b)))
    ((list '- a b) (- (eval-expr a) (eval-expr b)))
    ((cons 'neg (list a)) (- 0 (eval-expr a)))
    (n (where (number? n)) n)
    (_ (error "Invalid expression:" expr))))

(define math-ast '(+ (* 3 4) (neg 2)))
(println "Evaluating AST:" math-ast "=>" (eval-expr math-ast))

;; 2. Processing HTTP / API Responses (Record & Or-patterns)
(define (handle-response resp)
  (match resp
    ({status (or 200 201) data payload}
     (println "Success! Received data:" payload))
    ({status 404}
     (println "Not found!"))
    ({status code message msg} (where (>= code 500))
     (println "Server Error" code ":" msg))
    (_
     (println "Unhandled response:" resp))))

(handle-response {status 200 data "User Profile: Alice"})
(handle-response {status 404 url "/api/items"})
(handle-response {status 502 message "Bad Gateway"})

;; 3. Recursive List Processing with Rest Patterns & Cons
(define (sum-squares items)
  (match items
    ('() 0)
    ((cons h t) (+ (* h h) (sum-squares t)))))

(println "Sum of squares '(1 2 3 4):" (sum-squares '(1 2 3 4)))

;; 4. Rest pattern decomposition
(define (format-command cmd)
  (match cmd
    ((list verb arg1 & rest)
     (println "Command:" verb "| Primary Arg:" arg1 "| Extra Options:" rest))
    ((list verb)
     (println "Simple Command:" verb))
    (_
     (println "Unknown command format"))))

(format-command '(copy "src.txt" "dst.txt" --verbose --force))
(format-command '(status))

;; 5. `match-lambda` Shorthand
(define describe-shape
  (match-lambda
    ({type 'circle radius r}
     (* 3.14159 (* r r)))
    ({type 'rect width w height h}
     (* w h))
    (_ 0)))

(println "Area of circle:" (describe-shape {type 'circle radius 5}))
(println "Area of rectangle:" (describe-shape {type 'rect width 4 height 6}))
