import sqlite3,sys
c=sqlite3.connect(sys.argv[1]);c.executescript("create table customer(id integer,nome text,email text);insert into customer values(1,'Maria Silva','maria@x.com');");c.commit()
