ALTER TABLE asset_categories
    ADD COLUMN parent_category_id uuid REFERENCES asset_categories (category_id);

ALTER TABLE asset_categories
    DROP CONSTRAINT asset_categories_laboratory_id_name_key;

ALTER TABLE asset_categories
    ADD CONSTRAINT asset_categories_parent_not_self
    CHECK (parent_category_id IS NULL OR parent_category_id <> category_id);

CREATE UNIQUE INDEX idx_asset_categories_unique_sibling_name
    ON asset_categories (
        laboratory_id,
        COALESCE(parent_category_id, '00000000-0000-0000-0000-000000000000'::uuid),
        name
    );

CREATE INDEX idx_asset_categories_parent_category_id
    ON asset_categories (parent_category_id);
