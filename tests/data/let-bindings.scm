(define (bindings-that-do-not-fit-on-one-line input)
(let ([first-binding (compute-first-value input)] [second-binding (compute-second-value input)] [third-binding (compute-third-value input)])
(combine first-binding second-binding third-binding)))

(define (short-bindings input)
(let ([a (first input)] [b (second input)])
(combine a b (a-deliberately-long-call-that-forces-the-body-to-wrap input))))

(define (named-let input)
(let walk ([node input] [seen '()])
(if (null? node) seen (walk (cdr node) (cons (car node) seen)))))
