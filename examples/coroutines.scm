;; Cooperative coroutines in sel Lisp
;; This example demonstrates cooperative multitasking using co-create, co-yield, and co-resume.

(println "=== Sel Coroutines Demo: Producer-Consumer Generator ===")

;; A coroutine generator that yields squares of even numbers
(define make-even-squares-generator
  (lambda (max-num)
    (co-create
      (lambda (_)
        (define loop
          (lambda (current)
            (when (<= current max-num)
              (when (= (mod current 2) 0)
                (define square (* current current))
                (println "  [Generator] Yielding square of" current "->" square)
                (co-yield square))
              (loop (+ current 1)))))
        (loop 1)
        (println "  [Generator] Completed!")
        'done))))

;; Create a generator up to 10
(define gen (make-even-squares-generator 10))

(println "Generator state initially:" (co-state gen))

;; Consumer loop that pulls values from the generator
(define consume
  (lambda (g)
    (unless (co-dead? g)
      (define val (co-resume g nil))
      (unless (eq? val 'done)
        (println "  [Consumer] Received value from coroutine:" val)
        (consume g)))))

(println "Starting consumer loop:")
(consume gen)

(println "Generator state after completion:" (co-state gen))
