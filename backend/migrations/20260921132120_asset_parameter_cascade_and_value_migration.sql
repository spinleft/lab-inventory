-- 资产参数的删除级联,以及允许修改数据类型。
--
-- 1) 删除一个资产参数时,引用它的分配(资产类别/资产项)和已经录入的值都要一起消失。
--    这两条外键此前都是 NO ACTION,只要有人引用就删不掉(接口报 409)。
-- 2) 修改数据类型要求"父行先改、子行后改",而值的
--    (parameter_type_id, data_type) 外键原本是即时校验 —— 中间状态必然违反约束,
--    所以改成可延迟,校验推到 commit。ON DELETE CASCADE 同时补上,删参数即删值。
-- 3) 值行要保留迁移前的旧列:同一行里既能读到迁移前的值,也能读到迁移后的值。
--    因此 CHECK 只要求"当前 data_type 对应的列有值",不再要求其余列为 NULL。
--    读路径一直都按行 data_type 选列,旧列不会污染当前值。
-- 4) 按 parameter_type_id 查找此前没有索引,级联删除和新的读路径都靠它。
--
-- 值到选项的外键(asset_parameter_values_parameter_type_id_value_option_id_fkey)
-- 保持 NO ACTION:删掉资产已经选中的选项仍然报冲突,枚举的语义不变。

ALTER TABLE asset_parameter_assignments
    DROP CONSTRAINT asset_parameter_assignments_parameter_type_id_fkey;
ALTER TABLE asset_parameter_assignments
    ADD CONSTRAINT asset_parameter_assignments_parameter_type_id_fkey
        FOREIGN KEY (parameter_type_id)
        REFERENCES asset_parameter_types(parameter_type_id)
        ON DELETE CASCADE;

ALTER TABLE asset_parameter_values
    DROP CONSTRAINT asset_parameter_values_parameter_type_id_data_type_fkey;
ALTER TABLE asset_parameter_values
    ADD CONSTRAINT asset_parameter_values_parameter_type_id_data_type_fkey
        FOREIGN KEY (parameter_type_id, data_type)
        REFERENCES asset_parameter_types(parameter_type_id, data_type)
        ON DELETE CASCADE
        DEFERRABLE INITIALLY DEFERRED;

ALTER TABLE asset_parameter_values DROP CONSTRAINT asset_parameter_values_check;
ALTER TABLE asset_parameter_values ADD CONSTRAINT asset_parameter_values_check CHECK (
      (data_type = 'text'    AND value_text IS NOT NULL)
   OR (data_type = 'number'  AND value_number IS NOT NULL)
   OR (data_type = 'range'   AND value_range_start IS NOT NULL AND value_range_end IS NOT NULL AND value_range_start <= value_range_end)
   OR (data_type = 'boolean' AND value_boolean IS NOT NULL)
   OR (data_type = 'date'    AND value_date IS NOT NULL)
   OR (data_type = 'enum'    AND value_option_id IS NOT NULL)
);

CREATE INDEX idx_asset_parameter_assignments_parameter_type_id
    ON asset_parameter_assignments (parameter_type_id);

CREATE INDEX idx_asset_parameter_values_parameter_type_id
    ON asset_parameter_values (parameter_type_id);
