<template>

    <div class="form-input-wrapper">
        <div :class="'form-input' + (type === 'textarea' ? ' form-input--textarea' : '')">
            
            <textarea 
                v-if="type === 'textarea'" 
                :name="fieldname" 
                row="3"  
                :value="value" 
                @input="$emit('input', $event.target.value)" />

            <input 
                v-else 
                :name="fieldname" 
                :type="(type ? type :'text')" 
                :autocomplete="(autocomplete ? autocomplete : null)"
                :value="value" 
                @input="$emit('input', $event.target.value)" />

            <label class="form-input-label" :for="fieldname">{{ label }}</label>
            <span class="form-input-bar"></span>
        </div>
        <b class="error-message" v-if="errors">{{ errors }}</b>
    </div>

</template>

<script>
export default {
  props: ['type', 'label', 'fieldname', 'value', 'errors', 'autocomplete'] // Autocomplete reference https://developer.mozilla.org/en-US/docs/Web/HTML/Attributes/autocomplete#values
}
</script>

<style lang="css" scoped>
@import "~/assets/css/variables.css";

.form-input {
    position: relative;
    width: auto;
    display: block;
    min-height: 56px;
    background: #f8f8f8;
    box-shadow: inset 0 0 0 1px #d5d5d5;
    box-sizing: border-box;
    margin-top: 0.5em;
}

.form-input input, 
.form-input textarea {
    color: var(--default);
    background: transparent;
    position: absolute;
    top: 0;
    left: 0;
    font-size: 16px;
    font-family: var(--font);
    width: 100%;
    box-sizing: border-box;
    line-height: 46px;
    padding: 10px 0.8em 0;
    cursor: text;
    border-radius: 0;
    border: 0;
    outline: none;
    z-index: 3;
    box-shadow: inset 0 0 0 1px #d5d5d5;
}
.form-input textarea,
.form-input--textarea {
    height: 110px;
    line-height: 21px;
    resize: none;
}

.form-input textarea {
    padding-top: 24px;
    line-height: 21px;
}


.form-input input:focus, 
.form-input textarea:focus {
    outline: none;
}

.form-input input:focus ~ .form-input-bar:after,
.form-input textarea:focus ~ .form-input-bar:after {
    transform: scaleX(1);
}


.form-input label {
    text-align: left;
    position: absolute;
    top: 0;
    left: 0;
    color: var(--default);
    font-size: 16px;
    width: 100%;
    line-height: 56px;
    max-height: 56px;
    overflow: hidden;
    text-overflow: ellipsis;
    padding: 0 15px;
    z-index: 2;
    transition: 0.2s ease all; 
    transform-origin: 0 0;
    user-select: none;
}

.form-input-bar:after {
    content: ' ';
    position: absolute;
    bottom: 0;
    left: 0;
    height: 3px;
    background: var(--default);
    width: 100%;
    z-index: 9;
    transition: 0.2s all ease;
    transform-origin: center;
    transform: scaleX(0);
}

.isFilled label {
    transform: translate(2px,-8px) scale(0.75);
    font-weight: bold;
    opacity: 1;
}

.error-message {
    display: block;
    color: #c00b23;
    text-align: left;
    font-size: 13px;
}

/* Chrome autofill styles */
input:-webkit-autofill,
input:-webkit-autofill:hover, 
input:-webkit-autofill:focus,
textarea:-webkit-autofill,
textarea:-webkit-autofill:hover,
textarea:-webkit-autofill:focus,
select:-webkit-autofill,
select:-webkit-autofill:hover,
select:-webkit-autofill:focus {
    border: none;
    -webkit-text-fill-color: var(--default);
    -webkit-box-shadow: 0 0 0 1000px rgba(0, 149, 255, 0.15) inset;
    transition: background-color 5000s ease-in-out 0s;
}

</style>
