-- Postcard Vec<String> of enabled compendium sources; NULL means the default (SRD only).
ALTER TABLE campaigns ADD COLUMN sources BLOB;
