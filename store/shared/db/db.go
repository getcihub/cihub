package db

import (
	"database/sql"
	"runtime/debug"

	"github.com/jmoiron/sqlx"
)

type (
	// Locker represents an object that can be locked and unlocked.
	Locker interface {
		Lock()
		Unlock()
		RLock()
		RUnlock()
	}

	// Binder defines database field bindings.
	Binder interface {
		BindNamed(query string, arg interface{}) (string, []interface{}, error)
	}

	// Queryer defines a set of methods for
	// querying the database.
	Queryer interface {
		Query(query string, args ...interface{}) (*sql.Rows, error)
		QueryRow(query string, args ...interface{}) *sql.Row
	}

	// Execer interface defines a set of methods for executing
	// read and write commands against the database.
	Execer interface {
		Queryer
		Exec(query string, args ...interface{}) (sql.Result, error)
	}

	// Scanner represents an object that can be scanned for values.
	Scanner interface {
		Scan(dest ...interface{}) error
	}

	// DB is a pool of zero or more underlying connections to a database
	DB struct {
		conn   *sqlx.DB
		driver Driver
		locker Locker
	}
)

// View executes a function within the context of a managed read-only
// transaction. Any error that is returned from the function is returned
// from the View() method.
func (db *DB) View(fn func(Queryer, Binder) error) error {
	db.locker.RLock()
	err := fn(db.conn, db.conn)
	db.locker.RUnlock()
	return err
}

// Lock obtains a write lock to the database (sqlite only) and executes
// a function. Any error that is returned from the function is returned
// from the Lock() method.
func (db *DB) Lock(fn func(Execer, Binder) error) error {
	db.locker.Lock()
	err := fn(db.conn, db.conn)
	db.locker.Unlock()
	return err
}

// Update executes a function within the context of a read-write managed
// transaction. If no error is returned from the function then the
// transaction is committed. If an error is returned then the entire
// transaction is rolled back. Any error that is returned from the function
// or returned from the commit is returned from the Update() method.
func (db *DB) Update(fn func(Execer, Binder) error) (err error) {
	db.locker.Lock()
	defer db.locker.Unlock()

	tx, err := db.conn.Begin()
	if err != nil {
		return err
	}

	defer func() {
		if p := recover(); p != nil {
			err = tx.Rollback()
			debug.PrintStack()
		} else if err != nil {
			_ = tx.Rollback()
		} else {
			err = tx.Commit()
		}
	}()

	err = fn(tx, db.conn)
	return err
}

// Driver returns the name of the SQL driver.
func (db *DB) Driver() Driver {
	return db.driver
}

// Close closes the database connection.
func (db *DB) Close() error {
	return db.conn.Close()
}
