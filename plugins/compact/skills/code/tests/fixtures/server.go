//go:build !windows

// Package main is a small demo exercising Go's automatic semicolons.
package main

import (
	"errors"
	"fmt"
	"sort"
	"strings"
)

type Item struct {
	Name  string `json:"name,omitempty"`
	Price float64
	Tags  []string
}

type Store struct {
	items map[string]*Item
}

var ErrMissing = errors.New("missing")

const (
	A = iota
	B
	C
)

func NewStore() *Store {
	return &Store{items: map[string]*Item{}}
}

func (s *Store) Put(it *Item) { s.items[it.Name] = it }

func (s *Store) Get(name string) (*Item, error) {
	it, ok := s.items[name]
	if !ok {
		return nil, fmt.Errorf("get %q: %w", name, ErrMissing)
	}
	return it, nil
}

func (s *Store) Names() []string {
	out := make([]string, 0, len(s.items))
	for k := range s.items {
		out = append(out, k)
	}
	sort.Strings(out)
	return out
}

func main() {
	s := NewStore()
	s.Put(&Item{Name: "apple", Price: 0.5, Tags: []string{"fruit", "red"}})
	s.Put(&Item{
		Name:  "pear",
		Price: 0.75,
	})
	raw := `raw string
    with   spaces`
	x := 5
	x++
	y := x - -2 /* block comment */
	z := func(n int) int {
		return n * 2
	}(21)
	ch := make(chan int, 1)
	go func() { ch <- 3 }()
	v := <-ch
	var sb strings.Builder
	for i := 0; i < 3; i++ {
		sb.WriteString(fmt.Sprint(i))
	}
	_, err := s.Get("kiwi")
loop:
	for {
		switch {
		case v > 0:
			v--
			continue loop
		default:
			break loop
		}
	}
	defer fmt.Println("deferred")
	msg := strings.
		ToUpper("chained")
	fmt.Println(strings.Join(s.Names(), ","), raw, x, y, z, sb.String(), errors.Is(err, ErrMissing), A, B, C, msg)
}
