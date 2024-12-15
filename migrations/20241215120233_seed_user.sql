-- Add migration script here
INSERT INTO
    users (user_id, username, password_hash)
VALUES
    (
        'b8157ebc-ee57-43f7-95a0-d3eb593466c6',
        'admin',
        '$argon2id$v=19$m=15000,t=2,p=1$6+kKKUERbRBzN8cWNz3ugw$kE5K9iapT8a73HLhESmpvJ/ngQFm9scfv68fCruZVqE'
    )