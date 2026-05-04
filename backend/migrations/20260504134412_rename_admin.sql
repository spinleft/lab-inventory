-- Add migration script here
UPDATE users
SET
    username = 'root'
WHERE username = 'admin';