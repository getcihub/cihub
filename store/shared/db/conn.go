package db

import (
	"database/sql"
	"sync"
	"time"

	"github.com/jmoiron/sqlx"

	"github.com/getcihub/cihub/store/shared/migrate/postgres"
	"github.com/getcihub/cihub/store/shared/migrate/sqlite"
)

// Connect to a database and verify with a ping.
func Connect(driver Driver, datasource string, maxOpenConnections int) (*DB, error) {
	db, err := sql.Open(driver.String(), datasource)
	if err != nil {
		return nil, err
	}

	if err := pingDatabase(db); err != nil {
		return nil, err
	}

	if err := setupDatabase(db, driver); err != nil {
		return nil, err
	}

	// generally set to 0, user configured for larger installs
	db.SetMaxOpenConns(maxOpenConnections)

	var engine Driver
	var locker Locker
	switch driver {
	case Postgres:
		engine = Postgres
		locker = &nopLocker{}
	default:
		engine = Sqlite
		locker = &sync.RWMutex{}
	}

	return &DB{
		conn:   sqlx.NewDb(db, driver.String()),
		driver: engine,
		locker: locker,
	}, nil
}

// helper function to ping the database with backoff to ensure
// a connection can be established before we proceed with the
// database setup and migration.
func pingDatabase(db *sql.DB) (err error) {
	for i := 0; i < 30; i++ {
		err = db.Ping()
		if err == nil {
			return
		}
		time.Sleep(time.Second)
	}
	return
}

func setupDatabase(db *sql.DB, driver Driver) error {
	switch driver {
	case Postgres:
		return postgres.Migrate(db)
	default:
		return sqlite.Migrate(db)
	}
}
