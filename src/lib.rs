pub mod screwsat {
    use std::{
        collections::{HashMap, HashSet, VecDeque},
        time::{Duration, Instant},
        vec,
    };

    pub type Var = usize;
    pub type Lit = (Var, bool); //(0, true) means x0 and (0, false) means not x0.

    pub struct Solver {
        n: usize,                           // variables
        pub assigns: Vec<bool>,             // assignments
        clauses: Vec<Vec<Lit>>,             // all clauses(original + learnt)
        watchers: HashMap<Lit, Vec<usize>>, // clauses that may be conflicted or propagated if a `lit` is false.
        reason: Vec<Option<usize>>, // a clause index represents that a variable is forced to be assigned.
        level: Vec<usize>,          // decision level(0: unassigned, 1: minimum level)
        que: VecDeque<Var>,         //
        head: usize,
    }

    impl Solver {
        /// Enqueue a variable to assign a `value` to a boolean `assign`
        pub fn enqueue(&mut self, var: Var, assign: bool, reason: Option<usize>) {
            self.assigns[var] = assign;
            self.reason[var] = reason;
            self.level[var] = if let Some(last) = self.que.back() {
                self.level[*last]
            } else {
                1
            };
            self.que.push_back(var);
        }

        /// Add a new clause to `clauses` and watch a clause
        fn add_clause(&mut self, clause: &[Lit]) {
            let clause_idx = self.clauses.len();
            for &c in clause.iter() {
                self.watchers.entry(c).or_insert(vec![]).push(clause_idx);
            }
            self.clauses.push(clause.to_vec());
        }
        /// Propagate it by all enqueued values and check conflicts.
        /// If a conflict is detected, this function returns a conflicted clause index.
        /// `None` is no conflicts.
        fn propagate(&mut self) -> Option<usize> {
            while self.head < self.que.len() {
                let p = {
                    let v = self.que[self.head];
                    self.head += 1;
                    (v, !self.assigns[v])
                };

                if let Some(watcher) = self.watchers.get(&p) {
                    'next_clause: for &cr in watcher.iter() {
                        let mut cnt = 0;
                        //let clause = &mut self.clauses[*cr];
                        let len = self.clauses[cr].len();

                        for c in 0..len {
                            let (v, sign) = self.clauses[cr][c];
                            if self.level[v] == 0 {
                                // this variable hasn't been decided yet
                                self.clauses[cr].swap(c, 0);
                                cnt += 1;
                            } else if self.assigns[v] == sign {
                                // this clause is already satisfied
                                self.clauses[cr].swap(c, 0);
                                continue 'next_clause;
                            }
                        }
                        if cnt == 0 {
                            return Some(cr);
                        } else if cnt == 1 {
                            // Unit clause
                            let (var, sign) = self.clauses[cr][0];
                            debug_assert!(self.level[var] == 0);
                            // NOTE
                            // I don't know how to handle this borrowing problem. Please help me.
                            // self.enqueue(var, sign, Some(cr));

                            self.assigns[var] = sign;
                            self.reason[var] = Some(cr);
                            self.level[var] = if let Some(last) = self.que.back() {
                                self.level[*last]
                            } else {
                                1
                            };
                            self.que.push_back(var);
                        }
                    }
                }
            }
            None
        }
        /// Analyze a conflict clause and deduce a learnt clause to avoid a current conflict
        fn analyze(&mut self, mut confl: usize) {
            let mut que_tail = self.que.len() - 1;
            let mut checked_vars = HashSet::new();
            let current_level = self.level[self.que[que_tail]];

            let mut learnt_clause = vec![];
            let mut backtrack_level = 1;
            let mut same_level_cnt = 0;
            loop {
                for p in self.clauses[confl].iter() {
                    let (var, _) = *p;
                    // already checked
                    if !checked_vars.insert(var) {
                        continue;
                    }
                    debug_assert!(self.level[var] <= current_level);
                    if self.level[var] < current_level {
                        learnt_clause.push(*p);
                        backtrack_level = std::cmp::max(backtrack_level, self.level[var]);
                    } else {
                        same_level_cnt += 1;
                    }
                }

                // Find the latest a value that is checked
                while !checked_vars.contains(&self.que[que_tail]) {
                    que_tail -= 1;
                }

                same_level_cnt -= 1;
                // There is no variables that are at the conflict level
                if same_level_cnt <= 1 {
                    break;
                }
                // Next
                confl = self.reason[self.que[que_tail]].unwrap();
            }
            let p = self.que[que_tail];
            learnt_clause.push((p, !self.assigns[p]));

            // Cancel decisions until the level is less than equal to the backtrack level
            while let Some(p) = self.que.back() {
                if self.level[*p] > backtrack_level {
                    self.level[*p] = 0;
                    self.que.pop_back();
                } else {
                    break;
                }
            }
            // propagate it by a new learnt clause
            self.enqueue(p, !self.assigns[p], Some(self.clauses.len()));
            self.head = self.que.len() - 1;
            self.add_clause(&learnt_clause);
        }

        pub fn new(n: usize, clauses: &Vec<Vec<Lit>>) -> Solver {
            let mut solver = Solver {
                n,
                que: VecDeque::new(),
                head: 0,
                clauses: Vec::new(),
                reason: vec![None; n],
                level: vec![0; n],
                assigns: vec![false; n],
                watchers: HashMap::new(),
            };
            for clause in clauses.iter() {
                solver.add_clause(clause);
            }
            solver
        }
        /// msec is the time limit.
        /// Reaching the time limit returns `falsez
        pub fn solve(&mut self, msec: Option<u64>) -> bool {
            let start = Instant::now();
            loop {
                if let Some(msec) = msec {
                    // reach the time limit
                    if start.elapsed() > Duration::from_millis(msec) {
                        return false;
                    }
                }
                if let Some(confl) = self.propagate() {
                    //Conflict
                    let current_level = self.level[*self.que.back().unwrap()];
                    if current_level == 1 {
                        return false;
                    }
                    self.analyze(confl);
                } else {
                    // No Conflict
                    // Select a decision variable that isn't decided yet
                    let mut p = None;
                    for v in 0..self.n {
                        if self.level[v] == 0 {
                            p = Some(v);
                            break;
                        }
                    }
                    if let Some(p) = p {
                        self.enqueue(p, self.assigns[p], None);
                        self.level[p] += 1;
                    } else {
                        // all variables are selected. which means that a formula is satisfied
                        return true;
                    }
                }
            }
        }
    }
}