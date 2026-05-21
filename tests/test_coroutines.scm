;; Test cooperative coroutines

(println "--- Cooperative Coroutine Tests ---")

;; 1. Basic coroutine creation and resumption
(define generator
  (co-create
    (lambda (start)
      (println "Generator started with:" start)
      (define next-val (co-yield (+ start 10)))
      (println "Generator received resume arg:" next-val)
      (define final-val (co-yield (+ next-val 20)))
      (println "Generator completing...")
      (+ final-val 100))))

(assert (eq? (co-state generator) 'suspended))
(assert (not (co-dead? generator)))

;; Resume 1
(println "Resuming generator first time...")
(define res1 (co-resume generator 5))
(println "Generator yielded:" res1)
(assert (= res1 15))
(assert (eq? (co-state generator) 'suspended))

;; Resume 2
(println "Resuming generator second time...")
(define res2 (co-resume generator 30))
(println "Generator yielded:" res2)
(assert (= res2 50))
(assert (eq? (co-state generator) 'suspended))

;; Resume 3 - Run to completion
(println "Resuming generator third time...")
(define res3 (co-resume generator 1000))
(println "Generator returned:" res3)
(assert (= res3 1100))
(assert (eq? (co-state generator) 'dead))
(assert (co-dead? generator))

(println "Basic coroutine creation, yield, and resume: PASS")

;; 2. Interleaved cooperative execution (Fibonacci sequence generator)
(define make-fib-generator
  (lambda ()
    (co-create
      (lambda (_)
        (define loop
          (lambda (a b)
            (co-yield a)
            (loop b (+ a b))))
        (loop 0 1)))))

(define fib (make-fib-generator))
(assert (= (co-resume fib nil) 0))
(assert (= (co-resume fib nil) 1))
(assert (= (co-resume fib nil) 1))
(assert (= (co-resume fib nil) 2))
(assert (= (co-resume fib nil) 3))
(assert (= (co-resume fib nil) 5))
(assert (= (co-resume fib nil) 8))
(assert (= (co-resume fib nil) 13))

(assert (eq? (co-state fib) 'suspended))
(println "Interleaved fibonacci generator: PASS")

;; 3. Re-entry / nested resume protection and dead resume protection
(define self-resumer
  (co-create
    (lambda (self-ref)
      (co-resume self-ref 123))))

(define tr1 (attempt (co-resume self-resumer self-resumer)))
(assert (err? tr1))
(println "Caught expected re-entry error:")
(println (error-value tr1))

(define tr2 (attempt (co-resume generator 999)))
(assert (err? tr2))
(println "Caught expected dead resume error:")
(println (error-value tr2))

(println "Re-entry and dead resume protection: PASS")
