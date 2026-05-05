BEGIN;

CREATE TEMPORARY TABLE user_type_id_replacements (
    name TEXT PRIMARY KEY,
    user_type_id uuid NOT NULL UNIQUE
) ON COMMIT DROP;

INSERT INTO user_type_id_replacements (name, user_type_id)
VALUES
    ('owner', 'be551106-757f-4518-bad3-dde0665c9e35'),
    ('maintainer', '43f94f38-8c44-4bbe-8c9b-a177daaeb828'),
    ('user', '7f49552d-4f8e-42ab-8770-c02be8aeb049'),
    ('guest', '7f4decd8-c017-4368-b31f-bd1427058687');

INSERT INTO user_types (user_type_id, name, description)
SELECT
    replacements.user_type_id,
    '__replacement_' || replacements.name,
    user_types.description
FROM user_type_id_replacements AS replacements
INNER JOIN user_types ON user_types.name = replacements.name;

UPDATE users
SET user_type_id = replacements.user_type_id
FROM user_types
INNER JOIN user_type_id_replacements AS replacements ON replacements.name = user_types.name
WHERE users.user_type_id = user_types.user_type_id;

DELETE FROM user_types
USING user_type_id_replacements AS replacements
WHERE user_types.name = replacements.name
  AND user_types.user_type_id <> replacements.user_type_id;

UPDATE user_types
SET name = replacements.name
FROM user_type_id_replacements AS replacements
WHERE user_types.user_type_id = replacements.user_type_id;

COMMIT;
